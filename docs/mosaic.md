# The mosaic: a grid on the clock

A mosaic is the still grid of `strip_svg` driven by the clock of `timeline`.
Every state of a sequence lands in a cell of its own, and the turn that reaches
it plays inside that cell while the cube fades in. When the turn lands, the cell
keeps the still picture and the next cell takes over, so the figure draws itself
one cube per move and a cube's arrival *is* the move that produced it.

Rows share one clock, which is what the figure was written for: a search coming
at a solution from both ends.

## The model

`cube_core::export::mosaic`:

| | |
|---|---|
| `Track` | one row: a cube, the moves that walk away from it, where its cells sit, and when it starts |
| `Layout` | columns, padding, caption height, and whether unreached cells are outlined |
| `timeline(tracks, cam, style, palette, marks, layout, steps, ms)` | the `Timeline` the existing writers take |
| `meet(cube, forward, backward, pause)` | the three tracks of a two sided search |

A **slot** is one move of the shared clock, drawn in `steps` frames. A track
with `delay: d` and *k* moves holds its first cube from the start of slot *d*
and lands its last at the end of slot *d + k - 1*; the clock runs as long as the
slowest track. A slot in which nothing turns is drawn as a single still frame
labelled `wait`, so a pause costs one frame rather than `steps` identical ones.

`back: true` fills leftwards from `col`. `Track::rewound(cube, moves)` builds
such a track from the state the moves *arrive* at, reversing and inverting them,
which is the half of a search that sets out from the solved cube. `wrap: true`
carries a long sequence into the next row when the columns run out.

`meet` lays out three tracks: `forward` rightwards from the scramble on row 0,
`backward` leftwards from the state the whole solution reaches on row 1, and the
two halves together on row 2, delayed until the other two are done. Rows 0 and 1
end on the same state in the same column, which is the column the searches met
in; both carry a `meet` caption. This library does no bidirectional search of
its own: it takes the two halves as given.

## Output size

Every distinct cube drawing is written once into `Timeline::backdrop` as a
`<defs>` entry and placed per frame by `<use>`, so the frames themselves hold no
geometry. A mosaic therefore costs about what a plain animation of the same
sequence costs: the sexy move at `steps=6` is 322 KB as a mosaic against 329 KB
as an animation. A twenty move solution drawn as a two sided figure is a few
megabytes; `steps` is the knob that matters, `size` barely changes it.

The ids are prefixed with a hash of the tracks so that two mosaics on one page
do not reach into each other's drawings.

## Using it

Python:

```python
import magiccube as mc

# one sequence filling a grid
mc.mosaic(mc.Cube(), "R U R' U'", columns=5, size=150)

# a search meeting in the middle, from the two halves of a solution
short = mc.Cube.scrambled(seed=3, moves=6)
halves = str(short.solve()).split()
mc.mosaic(short, " ".join(halves[:3]), meet=" ".join(halves[3:]), size=130)

# rows placed by hand
mc.mosaic(tracks=[
    {"cube": scrambled, "word": forward, "row": 0, "col": 0, "first": "scrambled"},
    {"cube": solved, "word": backward, "row": 1, "col": 6, "back": True},
    {"cube": scrambled, "word": whole, "row": 2, "col": 0, "delay": 4},
])
```

`mosaic_player(...)` takes the same arguments and returns the self contained
HTML page instead, where the arrows step one slot of the clock, so a two sided
figure steps both of its rows at once.

Command line:

```sh
cargo run -p cube-export -- -s "R U R' U'" --mode mosaic --columns 5 -o grid.svg
cargo run -p cube-export -- --scramble 6 --seed 3 -s "F2 D B2" --meet "U' F' L" \
    --mode mosaic --size-px 130 -o meet.svg
```

`--mode mosaic-player` writes the HTML page, `--pause N` sets the beats before
the combined row, and `--no-ghosts` leaves unreached cells blank.

## Not in the app yet

The browser front end does not offer this export. The layout and timing live in
`cube-core`, and the app animates by calling `cube3d::inner` itself per frame,
so wiring it up later is a matter of driving `Lane`-style placement from the
app's own frame loop rather than reimplementing anything.
