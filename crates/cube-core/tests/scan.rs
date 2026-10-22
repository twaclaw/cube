//! Reading a cube back out of sampled colors.
//!
//! The camera is not here, so the samples are made the other way round: take
//! a state, paint every facelet the palette color of its face, then spoil the
//! paint the way a phone does -- sensor noise, a different exposure on every
//! face, and a white balance that pulls the whole picture warm or cold.

use cube_core::geom::Face;
use cube_core::palette::{Rgb, STANDARD};
use cube_core::scan::{self, Samples};
use cube_core::scramble::{Rng, scrambled};
use cube_core::{Cube, Permutation};
use proptest::prelude::*;

/// The colors a perfect camera would report.
fn clean(cube: &Cube) -> Samples {
    core::array::from_fn(|i| {
        let face = Face::ALL[i / 9];
        let (row, col) = (((i % 9) / 3) as u8, (i % 3) as u8);
        STANDARD.face(cube.get(face, row, col))
    })
}

/// A gain per channel and a brightness per face, which is most of what goes
/// wrong between the sticker and the pixel.
struct Camera {
    /// White balance: one multiplier per channel, shared by the whole shoot.
    balance: [f64; 3],
    /// Exposure of each face, since they are photographed one at a time.
    exposure: [f64; 6],
    /// Half-width of the noise added to every channel of every sample.
    noise: f64,
}

impl Camera {
    fn shoot(&self, clean: &Samples, rng: &mut Rng) -> Samples {
        core::array::from_fn(|i| {
            let gain = self.exposure[i / 9];
            let c = [clean[i].0, clean[i].1, clean[i].2];
            let mut out = [0u8; 3];
            for (k, v) in c.iter().enumerate() {
                let jitter = (rng.below(2001) as f64 / 1000.0 - 1.0) * self.noise;
                let value = *v as f64 * gain * self.balance[k] + jitter;
                out[k] = value.clamp(0.0, 255.0) as u8;
            }
            Rgb(out[0], out[1], out[2])
        })
    }
}

/// Every color nine times, whatever the samples were: the reading is always
/// a shape the rest of the program can draw.
fn counts(cube: &Cube) -> [usize; 6] {
    let mut out = [0; 6];
    for sticker in cube.stickers() {
        out[*sticker as usize] += 1;
    }
    out
}

#[test]
fn a_clean_shot_reads_back_exactly() {
    for seed in 0..40u64 {
        let (cube, _) = scrambled(3, 25, &mut Rng::new(seed));
        let reading = scan::read(&clean(&cube));
        assert_eq!(reading.cube, cube, "seed {seed}");
        assert!(
            reading.unsure.is_empty(),
            "seed {seed}: {:?} flagged on a clean shot",
            reading.unsure
        );
    }
}

#[test]
fn the_centers_keep_their_own_color() {
    // Whatever else a reading gets wrong, the center of a face is that face:
    // it is the center that names the color in the first place.
    let mut rng = Rng::new(7);
    for _ in 0..200 {
        let samples: Samples = core::array::from_fn(|_| {
            Rgb(
                rng.below(256) as u8,
                rng.below(256) as u8,
                rng.below(256) as u8,
            )
        });
        let reading = scan::read(&samples);
        for face in Face::ALL {
            assert_eq!(reading.cube.get(face, 1, 1), face);
        }
        assert_eq!(counts(&reading.cube), [9; 6]);
    }
}

#[test]
fn a_photographed_cube_survives_the_camera() {
    // Warm light, a stop of difference between the brightest and darkest
    // face, and noise worth a few levels: a phone indoors.
    let camera = Camera {
        balance: [1.08, 1.0, 0.88],
        exposure: [1.0, 0.82, 0.95, 1.12, 0.9, 1.05],
        noise: 9.0,
    };
    let mut rng = Rng::new(99);
    for seed in 0..60u64 {
        let (cube, _) = scrambled(3, 25, &mut Rng::new(seed));
        let samples = camera.shoot(&clean(&cube), &mut rng);
        assert_eq!(scan::read(&samples).cube, cube, "seed {seed}");
    }
}

proptest! {
    /// Nothing the camera reports, however bad, produces something that is
    /// not a cube's worth of stickers.
    #[test]
    fn any_samples_read_as_nine_of_each(seed in any::<u64>()) {
        let mut rng = Rng::new(seed);
        let samples: Samples = core::array::from_fn(|_| {
            Rgb(rng.below(256) as u8, rng.below(256) as u8, rng.below(256) as u8)
        });
        let reading = scan::read(&samples);
        prop_assert_eq!(counts(&reading.cube), [9; 6]);
        prop_assert_eq!(reading.cube.n(), 3);
    }

    /// A state the camera saw cleanly is a state the solver will accept.
    #[test]
    fn a_clean_reading_is_a_reachable_state(seed in any::<u64>()) {
        let (cube, _) = scrambled(3, 25, &mut Rng::new(seed));
        let reading = scan::read(&clean(&cube));
        prop_assert!(cube_core::solver::check(&reading.cube).is_ok());
        prop_assert!(Permutation::of(&reading.cube).is_some());
    }
}
