import marimo

__generated_with = "0.25.1"
app = marimo.App()


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    # Drawing a Rubik's cube

    The figures and animations in the  [web app](https://github.com/twaclaw/cube) are also available as a Python API.
    Every picture below is SVG, drawn by the same Rust renderer. Build it with `make python`, open this notebook with `make notebook`.
    This notebook is a demonstration of the capabilities of this API.
    """)


@app.cell
def _():
    from pathlib import Path

    import magiccube as mc
    import marimo as mo

    HERE = Path(__file__).parent if "__file__" in globals() else Path.cwd()
    FIGURES = HERE / "figures"
    FIGURES.mkdir(exist_ok=True)
    return FIGURES, mc, mo


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    ## The two views

    A cube, turned by the sexy move, seen in three dimensions. Drag is for the app;
    here the camera is just an angle.
    """)


@app.cell
def _(mc):
    cube = mc.Cube()
    cube.apply("R U R' U'")
    cube.view3d(yaw=38, pitch=24)
    return (cube,)


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    The same state, unfolded. The numbers are the SageMath facelet numbering: 1 to 48, faces in the order U, L, F, R, B, D, row by
    row, centers skipped because they never leave their face.
    """)


@app.cell
def _(cube):
    cube.net(numbers=True)


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    ## The state as a permutation

    A cube state *is* a permutation of those 48 numbered facelets, and the sexy
    move is a short one: two three-cycles of edges and two of corners, repeating
    after six goes.
    """)


@app.cell
def _(cube, mo):
    perm = cube.permutation()
    mo.md(f"`{perm.notation}`  — order {perm.order}, moving {perm.moved} of {len(perm)} facelets")
    return (perm,)


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    Drawn, with every number in the colour of the face it belongs to, the shape of
    the maneuver is easier to see than in the text.
    """)


@app.cell
def _(perm):
    perm.draw(width=700)


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    ## A blank cube, marked up

    Pass a dict of cubie (or facelet) to colour. The selectors return exactly that
    kind of dict, and dicts merge with `|`, so naming the three kinds of cubie is
    one line.
    """)


@app.cell
def _(mc):
    kinds = mc.corners("red") | mc.edges("blue") | mc.centers("gold")
    mc.Cube().net(blank=True, marks=kinds)
    return (kinds,)


@app.cell
def _(kinds, mc):
    mc.Cube().view3d(blank=True, marks=kinds, yaw=30, pitch=26)


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    A corner shows three facelets, an edge two, a center one -- which is the whole
    reason the cube has 8 corners, 12 edges and 6 centers rather than 26 of one
    thing.

    ## What phase 2 of Kociemba's algorithm can move

    Phase 2 may only turn `U`, `D` and the half turns `R2 L2 F2 B2`. Color the
    *orbits* of those moves -- the groups of slots that can reach each other -- and
    the constraint becomes visible: the four slice edges are an orbit of their own.
    """)


@app.cell
def _(mc):
    phase2 = mc.orbits(["U", "D", "R2", "L2", "F2", "B2"])
    mc.Cube().net(blank=True, marks=phase2)
    return (phase2,)


@app.cell
def _(mc, phase2):
    mc.Cube().view3d(blank=True, marks=phase2, yaw=38, pitch=24)


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    Green is the four slice edges, blue the eight U and D edges, red the eight
    corners. No move of phase 2 can carry a green cubie out of the middle layer,
    which is exactly why phase 1 has to put them there first.

    ## What a move leaves alone

    The same idea applied to a single maneuver: red for the cubies it carries
    somewhere else, pale for the ones it leaves where they are.
    """)


@app.cell
def _(mc):
    sexy = "R U R' U'"
    mc.Cube().net(blank=True, marks=mc.Cube().disturbed(sexy, "red") | mc.Cube().fixed(sexy, "#e7e7ec"))
    return (sexy,)


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    Seven cubies move, nineteen stay. That is what makes short commutators useful:
    they do something small to a cube that is otherwise finished.

    ## A maneuver, step by step
    """)


@app.cell
def _(mc, sexy):
    mc.strip(mc.Cube(), sexy, columns=5, size=190)


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    ## Solving

    Kociemba's two-phase algorithm, in the same module. Phase 1 drives the cube
    into the subgroup the picture above describes; phase 2 finishes inside it.
    """)


@app.cell
def _(mc, mo):
    scrambled = mc.Cube.scrambled(seed=11)
    solution = scrambled.solve()
    mo.md(f"""
    | | |
    |---|---|
    | scramble | `{scrambled.state}` |
    | phase 1 | `{solution.phase1}` |
    | phase 2 | `{solution.phase2}` |
    | total | {solution.length} moves, found in {solution.nodes} nodes |
    """)
    return scrambled, solution


@app.cell
def _(mc, scrambled, solution):
    mc.strip(scrambled, solution.moves, columns=7, size=170)


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    ## An animation

    An animated SVG: it plays inside an `<img>`, so it survives into Markdown.
    """)


@app.cell
def _(mc):
    film = "R U R' U'"
    mc.animation(mc.Cube(), film, steps=6)
    return (film,)


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    The same maneuver as a self-contained HTML page, with play, step and a
    scrubber. It carries its own frames, so it needs nothing to run.
    """)


@app.cell
def _(film, mc):
    mc.player(mc.Cube(), film)


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    And the one export that stays interactive: an `<iframe>` of the published
    app, which draws the cube itself and can be dragged, stepped and solved.
    """)


@app.cell
def _(mc, scrambled):
    mc.embed(scrambled, "R U R' U'", numbers=True, views="3d", height=380)


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    ## Saving the figures

    The purpose of the `make post` command is to export the notebook to Markdown, so that 
    I (or someone else) can use it in an interactive book or blog post.

    `marimo export md` writes the notebook's **source**, not its outputs, so a post
    built this way saves its figures and links to them from a plain Markdown cell.
    """)


@app.cell
def _(FIGURES, cube, film, mc, perm, phase2, scrambled, solution):
    saved = {
        "net": cube.net(numbers=True).save(str(FIGURES / "net.svg")),
        "phase2": mc.Cube().net(blank=True, marks=phase2).save(str(FIGURES / "phase2.svg")),
        "cycles": perm.draw(width=700).save(str(FIGURES / "cycles.svg")),
        "solution": mc.strip(scrambled, solution.moves, columns=7, size=170).save(str(FIGURES / "solution.svg")),
        "film": mc.animation(mc.Cube(), film, steps=6).save(str(FIGURES / "sexy-move.svg")),
        "player": mc.player(mc.Cube(), film).save(str(FIGURES / "sexy-move.html")),
    }
    saved


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    The cell below is a literal Markdown cell: no f-string, so the exporter keeps
    it as Markdown and the links point at the files just written.

    ![The net, numbered](figures/net.svg)

    ![What phase 2 can move](figures/phase2.svg)

    An animated SVG works in the same way, because it plays inside an `<img>`:

    ![The sexy move, animated](figures/sexy-move.svg)

    The HTML player needs an iframe rather than an image tag, which Markdown
    passes through untouched:

    <iframe src="figures/sexy-move.html" width="100%" height="460" style="border:0"></iframe>
    """)


if __name__ == "__main__":
    app.run()
