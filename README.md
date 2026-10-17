# cube

A Rubik's cube representation.

## What it does

- **Two views, one state.** Drag the 3D cube to turn it, or pick a named view.
  Zoom is off until you tick the box, so a scroll never hijacks the page.
- **Maneuvers.** Type a sequence and press enter to load it; nothing moves
  until you say so. Play it, step a move at a time, jump to any move by
  clicking it, or apply the rest at once. Turns are animated: the layer really
  rotates, and the cut it opens shows the dark body underneath.
- **Scramble / solved.** One button for a random state, one for the solved
  cube, and a third that the solver will fill in later.
- **Facelet numbers** on the net, on by default, in the SageMath numbering the
  notebooks use (1..48, U L F R B D, centers skipped). `?numbers=0` or the
  checkbox turns them off.
- **Highlights.** Click a facelet to ring it, or list numbers such as
  `1,3,8-11`. This is how the notebooks mark the facelets that break an
  invariant of the group `H = <U, D, R², L², F², B²>`.
- **Exports** for a blog, all from the same renderer (see below).
- **Any cube size.** The model and both renderers are written for n×n×n; the
  size selector goes from 2 to 7. Only the future solver is 3x3 only.

## Notation

One parser reads the three notations this project cares about, and they can be
mixed:

| written | means |
|---|---|
| `U D L R F B` | quarter turn of that face, clockwise |
| `u d l r f b` | the same turn counter-clockwise, so `rfu` is `R' F' U'` |
| `'` `2` `^2` `^-1` | inverse, half turn, SageMath style exponents |
| `M E S` | middle slice, following L, D and F |
| `*F` `>F` | slice and whole-cube turn, as in geometer.org's *Rubik* |
| `x y z` | whole-cube turns |
| `Rw` `2R` `2-3R` | wide and deep layers, for bigger cubes |
| `(R U)3` `(R U)'` | repeat a group, or invert it |

`R^2*U*F^-1` (the notebooks' SageMath product) and `R2 U F'` (WCA) both parse,
as does the crammed `rfu` of the *Rubik* macro language. A `*` between two
moves is read as the SageMath product; a `*` that starts a move is the slice
prefix.

## Exports

| export | what it is good for |
|---|---|
| 3D view, SVG | a still of the current state and camera |
| net, SVG | the flat diagram, with numbers and rings if shown |
| every step, one SVG | the whole maneuver as a grid of labeled states |
| animated SVG | SMIL flipbook; works inside `<img>`, so it drops into Markdown |
| animation, HTML page | self-contained player with play, step and scrub |
| embed snippet | an `<iframe>` of the live app, state and all, still interactive |

The same drawings come out of the command line exporter, which is what a blog
build should call:

```sh
cargo run -p cube-export -- --help
cargo run -p cube-export -- -s "R U R' U'" --mode strip -o steps.svg
cargo run -p cube-export -- --mode net --numbers --highlight 1,3,8-11 -o net.svg
```

The app reads its own state from the query string, which is what the embed
snippet uses: `?n=3&seq=R+U+R%27+U%27&cam=38,24&numbers=1&hl=1,3&embed=1&autoplay=1`.
`views=3d` or `views=net` shows one view only; `controls=1` keeps the console
in an embed.

## Running it

```sh
make dev                                           # live reload, http://127.0.0.1:8088
make preview PUBLIC_URL=/cube/ PREVIEW_PORT=...    # the release build, served from that path
make test                                          # unit, property and SageMath comparison tests
make build PUBLIC_URL=/cube/                       # the bundle GitHub Pages serves
```

The toolchain on your `PATH` needs the wasm target
(`rustup target add wasm32-unknown-unknown`); the Makefile puts `~/.cargo/bin`
first so rustup's toolchain wins over a package-manager Rust that has no wasm
std. Pushing to `main` builds and publishes through
`.github/workflows/pages.yml`.

## How it is put together

```
crates/cube-core     state, moves, notation, both SVG renderers, exports
crates/cube-web      the browser front end (wasm-bindgen and web-sys only)
crates/cube-export   the same exports from the command line
```

- **Geometry.** A facelet is a point on a doubled, centered integer lattice plus
  an outward normal, so a quarter turn is an exact integer rotation. One
  `Turn { axis, lo, hi, quarters }` covers face turns, wide turns, slices and
  whole-cube rotations, at any size. Nothing is tabulated per move.
- **3D rendering.** Perspective projection, back-face culling, painter's
  algorithm. Each group of cubies that moves together sits on a dark body box,
  which fills the grooves and the face a turn cuts open; inner cubie faces are
  then never drawn, which halves the output.
- **Framing.** The scale comes from the sphere the cube turns inside, so the
  view neither breathes while you drag nor clips a layer at 45 degrees.
- **Correctness.** The move permutations and the cycle notation are checked
  against SageMath's `CubeGroup`, string for string (`make vectors` regenerates
  the table), and the rest is property
  tested with proptest: four quarter turns are the identity, a maneuver is
  undone by its reverse, colors are never created or destroyed, the numbering
  is a bijection, the drawing always fits its viewBox, a face-on view shows
  exactly one face, and so on.


## Credits

The facelet numbering, the net layout and the color scheme follow SageMath's
`sage.groups.perm_gps.cubegroup`. The macro language and the slice and whole-cube prefixes come from Tom Davis's [*Rubik*](http://www.geometer.org/rubik/) and its user guide.
