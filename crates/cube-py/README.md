# magic-cube

Rubik's cube drawings from Python: the 3D view, the unfolded net, cycle
notation, animations and a solver. The drawing is done by a Rust renderer,
compiled in, so there is nothing to install beside the wheel.

```sh
pip install magic-cube
```

```python
import magic_cube as mc

cube = mc.Cube()
cube.apply("R U R' U'")
cube.view3d(yaw=38, pitch=24)      # an SVG, drawn in a notebook
```

> **Note on the name.** The import is `magic_cube`, with an underscore. The
> name `magiccube` on PyPI is an unrelated cube library.

The same renderer runs in the browser as a WebAssembly app --
**[try it online](https://twaclaw.github.io/cube/)** -- and the long form of
all of this is the book
**[Magic Cube](https://twaclaw.github.io/magic-cube/)**.

## What it draws

Every figure is a `Drawing`: `.svg` or `.text` for the markup, `.save(path)`
to write it out, and `_repr_html_` so a notebook shows it without being asked.

| | |
|---|---|
| `cube.view3d(...)` | the cube in three dimensions, any camera |
| `cube.net(numbers=True, ...)` | the unfolded net, optionally numbered |
| `cube.cycles()` | the state as disjoint cycles, each number drawn as its facelet |
| `mc.strip(cube, word, columns=6)` | every state of a maneuver, side by side |
| `mc.animation(cube, word)` | an animated SVG, which plays inside an `<img>` |
| `mc.player(cube, word)` | a self contained HTML page with play, step and scrub |
| `mc.mosaic(cube, word)` | a maneuver animated into a grid, one cube per move |
| `mc.embed(cube, word)` | an `<iframe>` of the published app, still interactive |
| `mc.table_tree(...)`, `mc.search_tree(...)` | the solver's tables and searches, as trees |

Marks colour individual cubies or facelets, and every renderer takes them:

```python
sexy = "R U R' U'"
mc.Cube().net(blank=True,
              marks=mc.Cube().disturbed(sexy, "red")
                   | mc.Cube().fixed(sexy, "#e7e7ec"))
```

`mc.corners()`, `mc.edges()`, `mc.centers()`, `mc.cubies([...])`,
`mc.facelets([...])` and `mc.orbits([...])` all build such dicts, keyed by
cubie name or facelet number.

## The cube itself

```python
cube = mc.Cube.scrambled(seed=11)   # reproducible
solution = cube.solve()             # Kociemba's two phase algorithm
solution.moves, solution.length, solution.nodes
```

`Cube(n=5)` for other sizes -- the renderer takes any `n`, the solver is 3x3
only. `cube.state` is the facelet string `Cube(state=...)` reads back.

`cube.permutation()` gives the state as a permutation of the 48 numbered
facelets, in SageMath's numbering, with `.cycles`, `.order`, `.cycle_type`,
`.moved` and `.notation`.

## Notation

The parser takes several notations, mixed if you like.

| written | means |
|---|---|
| `U D L R F B` | quarter turn of that face, clockwise |
| `u d l r f b` | the same turn counter-clockwise, so `rfu` is `R' F' U'` |
| `'` `2` `^2` `^-1` | inverse, half turn, SageMath style exponents |
| `M E S` | middle slice, following L, D and F |
| `x y z` | whole-cube turns |
| `Rw` `2R` `2-3R` | wide and deep layers, for bigger cubes |
| `(R U)3` `(R U)'` | repeat a group, or invert it |

So `R2 U F'`, `R^2*U*F^-1` and `rfu`-style crammed input all parse.

## License

MIT. Source at [github.com/twaclaw/cube](https://github.com/twaclaw/cube).
