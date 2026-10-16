//! Ground truth taken from SageMath's `CubeGroup`, which uses the same facelet
//! numbering as the notebooks: faces in the order U, L, F, R, B, D, row major,
//! centers left out, so 1..=48 on a 3x3.
//!
//! Regenerate with `make vectors`.

/// `(maneuver, facelet colors after applying it to a solved cube)`
pub const CASES: &[(&str, &str)] = &[
    ("U", "uuuuuuuuffflllllrrrfffffbbbrrrrrlllbbbbbdddddddd"),
    ("U'", "uuuuuuuubbbllllllllffffffffrrrrrrrrbbbbbdddddddd"),
    ("U2", "uuuuuuuurrrlllllbbbffffflllrrrrrfffbbbbbdddddddd"),
    ("L", "buububuulllllllluffufuffrrrrrrrrbbdbdbbdfddfdfdd"),
    ("L'", "fuufufuulllllllldffdfdffrrrrrrrrbbububbubddbdbdd"),
    ("L2", "duududuullllllllbffbfbffrrrrrrrrbbfbfbbfuddududd"),
    ("F", "uuuuullllldldlldffffffffurrururrbbbbbbbbrrrddddd"),
    ("F'", "uuuuurrrllululluffffffffdrrdrdrrbbbbbbbblllddddd"),
    ("F2", "uuuuudddllrlrllrfffffffflrrlrlrrbbbbbbbbuuuddddd"),
    ("R", "uufufuufllllllllffdfdffdrrrrrrrrubbububbddbdbddb"),
    ("R'", "uububuubllllllllffufuffurrrrrrrrdbbdbdbbddfdfddf"),
    ("R2", "uududuudllllllllffbfbffbrrrrrrrrfbbfbfbbddududdu"),
    ("B", "rrruuuuuullulullffffffffrrdrdrrdbbbbbbbbdddddlll"),
    ("B'", "llluuuuudlldldllffffffffrrururrubbbbbbbbdddddrrr"),
    ("B2", "ddduuuuurllrlrllffffffffrrlrlrrlbbbbbbbbddddduuu"),
    ("D", "uuuuuuuulllllbbbffffflllrrrrrfffbbbbbrrrdddddddd"),
    ("D'", "uuuuuuuulllllffffffffrrrrrrrrbbbbbbbbllldddddddd"),
    ("D2", "uuuuuuuulllllrrrfffffbbbrrrrrlllbbbbbfffdddddddd"),
    ("R U", "uuuuufffffdlllllrrrfdffdubbrrrrrlllububbddbdbddb"),
    (
        "R U R' U'",
        "uulufuufblllllllffdfufffrrubrurrbrrbbbbbddrddddd",
    ),
    ("F L D", "buubublllllllbbruffufdddurrurlffbbdbdurrfffdrddr"),
    (
        "U R2 D' F2 L2",
        "dudddduurffrlrfflbbbflrrlrrlrfbbflbfbllbuuuududd",
    ),
    (
        "B' D2 L F U' R B2 D",
        "bdblffbflbdururlrdrbdulruuulrbfdrfddllubffdurlbf",
    ),
    (
        "L L L L",
        "uuuuuuuullllllllffffffffrrrrrrrrbbbbbbbbdddddddd",
    ),
    (
        "R L R' L'",
        "uuuuuuuullllllllffffffffrrrrrrrrbbbbbbbbdddddddd",
    ),
];
