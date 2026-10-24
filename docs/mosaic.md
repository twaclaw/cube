# The mosaic: a grid on the clock

A mosaic is the still grid of `strip_svg` driven by the clock of `timeline`.
Every state of a sequence lands in a cell of its own, and the turn that reaches
it plays inside that cell while the cube fades in. When the turn lands, the cell
keeps the still picture and the next cell takes over, so the figure draws itself
one cube per move and a cube's arrival *is* the move that produced it.

Rows share one clock but need not run at one speed, which is what the figure
was written for: a search coming at a solution from both ends.

## The model

`cube_core::export::mosaic`:

| | |
|---|---|
| `Track` | one row: a cube, the moves that walk away from it, where its cells sit, when it starts and how fast it goes |
| `Pace` | the figure's own milliseconds per turn, rest on a landed state, and frames per turn |
| `Layout` | columns, padding, caption height, numbering, and whether unreached cells are outlined |
| `timeline(tracks, cam, style, palette, marks, layout, pace)` | the `Timeline` the existing writers take |
| `meet(cube, forward, backward, pause)` | the three tracks of a two sided search |

Time is milliseconds throughout, measured from the start of the figure. A track
appears, rests, turns, rests, turns, and so on, so the move it is on is read
straight off the clock: `start = delay * pace.beat()`, and move *i* swings
between `start + hold + i * (ms + hold)` and the end of that turn. A track that
sets its own `ms` or `hold` consumes the shared clock at its own rate, so one
row can crawl while another races; `delay` is still counted in turns of the
figure's pace, which is what makes "wait for the other rows" easy to write.

Frames are cut at the moments that matter: every arrival, every landing, and
`steps` samples across each turn. Consecutive frames whose drawing is identical
are merged into one held longer, so a rest on a landed state, and a row waiting
for two others to finish, cost time rather than frames.

`back: true` fills leftwards from `col`. `Track::rewound(cube, moves)` builds
such a track from the state the moves *arrive* at, reversing and inverting them,
which is the half of a search that sets out from the solved cube. `wrap: true`
carries a long sequence into the next row when the columns run out.

Once every track has played out, the cells a track names in `glow` light up
over a beat of their own: a wash of colour under the cube, wider than it is, so
what shows is a halo round its silhouette. `meet` lights the pair its two
searches land on, which is the whole point of the figure; nothing else lights
unless asked. The colour is `Layout::glow`, defaulting to the palette's
highlight.

Captions come from the moves, as `3. R`, unless something says otherwise:
`labels` writes one string of your own per move in place of the move's name,
`Layout::numbered` drops the `3. `, `first` and `last` annotate the ends of a
row, and `title` names the whole row in a margin down the left, which is sized
from the longest name and disappears when no row has one. Labels are plain text
for now; the hook for setting them is where TeX would eventually go.

`meet` lays out three tracks: `forward` rightwards from the scramble on row 0,
`backward` leftwards from the state the whole solution reaches on row 1, and the
two halves together on row 2, delayed until the other two are done. Rows 0 and 1
end on the same state in the same column, which is the column the searches met
in; both carry a `meet` caption. The rows are named forwards, backwards and
together, which the caller can overwrite. This library does no bidirectional
search of its own: it takes the two halves as given.

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
mc.mosaic(short, " ".join(halves[:3]), meet=" ".join(halves[3:]), size=130,
          hold=600, titles=["from the scramble", "from solved", "both halves"])

# rows placed by hand, one of them crawling while the other races
mc.mosaic(tracks=[
    {"cube": scrambled, "word": forward, "row": 0, "col": 0,
     "title": "forwards", "first": "scrambled", "ms": 150},
    {"cube": solved, "word": backward, "row": 1, "col": 6, "back": True,
     "title": "backwards", "ms": 700},
    {"cube": scrambled, "word": whole, "row": 2, "col": 0, "delay": 4,
     "title": "together"},
])

# something of your own under each cube, and no step numbers
mc.mosaic(tracks=[{"cube": mc.Cube(), "word": "R U R' U'",
                   "labels": ["pull", "turn", "push", "back"]}], numbered=False)

# the cube a row ends on, lit in a colour of your own
mc.mosaic(c, "R U R' U'", glow=True, glow_color="gold")
mc.mosaic(tracks=[{"cube": c, "word": "R U R' U'", "glow": [2, 4]}])
```

`ms` is how long a turn takes and `hold` how long a landed state is left up
before the next turn starts; both default per figure and can be overridden per
row. Leaving `hold` alone rests for a little under half a turn.

`mosaic_player(...)` takes the same arguments and returns the self contained
HTML page instead, where the arrows step one slot of the clock, so a two sided
figure steps both of its rows at once.

Command line:

```sh
cargo run -p cube-export -- -s "R U R' U'" --mode mosaic --columns 5 -o grid.svg
cargo run -p cube-export -- --scramble 6 --seed 3 -s "F2 D B2" --meet "U' F' L" \
    --mode mosaic --size-px 130 -o meet.svg
```

`--mode mosaic-player` writes the HTML page, `--pause N` sets the turns to wait
before the combined row, `--hold N` the rest on each landed state, `--titles`
names the rows, `--unnumbered` drops the step numbers, `--glow #rrggbb` and
`--no-glow` control the light on the pair two searches meet on, and
`--no-ghosts` leaves unreached cells blank.

## Not in the app yet

The browser front end does not offer this export. The layout and timing live in
`cube-core`, and the app animates by calling `cube3d::inner` itself per frame,
so wiring it up later is a matter of driving `Lane`-style placement from the
app's own frame loop rather than reimplementing anything.
