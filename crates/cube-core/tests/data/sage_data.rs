//! Ground truth taken from SageMath's `CubeGroup`, which uses the same facelet
//! numbering as the notebooks: faces in the order U, L, F, R, B, D, row major,
//! centers left out, so 1..=48 on a 3x3.
//!
//! Regenerate with `make vectors`.

/// `(maneuver, facelet colors after applying it, the same state as a permutation)`
// One case per line reads better than what rustfmt would do with it.
#[rustfmt::skip]
pub const CASES: &[(&str, &str, &str)] = &[
    ("U", "uuuuuuuuffflllllrrrfffffbbbrrrrrlllbbbbbdddddddd", "(1,3,8,6)(2,5,7,4)(9,33,25,17)(10,34,26,18)(11,35,27,19)"),
    ("U'", "uuuuuuuubbbllllllllffffffffrrrrrrrrbbbbbdddddddd", "(1,6,8,3)(2,4,7,5)(9,17,25,33)(10,18,26,34)(11,19,27,35)"),
    ("U2", "uuuuuuuurrrlllllbbbffffflllrrrrrfffbbbbbdddddddd", "(1,8)(2,7)(3,6)(4,5)(9,25)(10,26)(11,27)(17,33)(18,34)(19,35)"),
    ("L", "buububuulllllllluffufuffrrrrrrrrbbdbdbbdfddfdfdd", "(1,17,41,40)(4,20,44,37)(6,22,46,35)(9,11,16,14)(10,13,15,12)"),
    ("L'", "fuufufuulllllllldffdfdffrrrrrrrrbbububbubddbdbdd", "(1,40,41,17)(4,37,44,20)(6,35,46,22)(9,14,16,11)(10,12,15,13)"),
    ("L2", "duududuullllllllbffbfbffrrrrrrrrbbfbfbbfuddududd", "(1,41)(4,44)(6,46)(9,16)(10,15)(11,14)(12,13)(17,40)(20,37)(22,35)"),
    ("F", "uuuuullllldldlldffffffffurrururrbbbbbbbbrrrddddd", "(6,25,43,16)(7,28,42,13)(8,30,41,11)(17,19,24,22)(18,21,23,20)"),
    ("F'", "uuuuurrrllululluffffffffdrrdrdrrbbbbbbbblllddddd", "(6,16,43,25)(7,13,42,28)(8,11,41,30)(17,22,24,19)(18,20,23,21)"),
    ("F2", "uuuuudddllrlrllrfffffffflrrlrlrrbbbbbbbbuuuddddd", "(6,43)(7,42)(8,41)(11,30)(13,28)(16,25)(17,24)(18,23)(19,22)(20,21)"),
    ("R", "uufufuufllllllllffdfdffdrrrrrrrrubbububbddbdbddb", "(3,38,43,19)(5,36,45,21)(8,33,48,24)(25,27,32,30)(26,29,31,28)"),
    ("R'", "uububuubllllllllffufuffurrrrrrrrdbbdbdbbddfdfddf", "(3,19,43,38)(5,21,45,36)(8,24,48,33)(25,30,32,27)(26,28,31,29)"),
    ("R2", "uududuudllllllllffbfbffbrrrrrrrrfbbfbfbbddududdu", "(3,43)(5,45)(8,48)(19,38)(21,36)(24,33)(25,32)(26,31)(27,30)(28,29)"),
    ("B", "rrruuuuuullulullffffffffrrdrdrrdbbbbbbbbdddddlll", "(1,14,48,27)(2,12,47,29)(3,9,46,32)(33,35,40,38)(34,37,39,36)"),
    ("B'", "llluuuuudlldldllffffffffrrururrubbbbbbbbdddddrrr", "(1,27,48,14)(2,29,47,12)(3,32,46,9)(33,38,40,35)(34,36,39,37)"),
    ("B2", "ddduuuuurllrlrllffffffffrrlrlrrlbbbbbbbbddddduuu", "(1,48)(2,47)(3,46)(9,32)(12,29)(14,27)(33,40)(34,39)(35,38)(36,37)"),
    ("D", "uuuuuuuulllllbbbffffflllrrrrrfffbbbbbrrrdddddddd", "(14,22,30,38)(15,23,31,39)(16,24,32,40)(41,43,48,46)(42,45,47,44)"),
    ("D'", "uuuuuuuulllllffffffffrrrrrrrrbbbbbbbbllldddddddd", "(14,38,30,22)(15,39,31,23)(16,40,32,24)(41,46,48,43)(42,44,47,45)"),
    ("D2", "uuuuuuuulllllrrrfffffbbbrrrrrlllbbbbbfffdddddddd", "(14,30)(15,31)(16,32)(22,38)(23,39)(24,40)(41,48)(42,47)(43,46)(44,45)"),
    ("R U", "uuuuufffffdlllllrrrfdffdubbrrrrrlllububbddbdbddb", "(1,3,38,43,11,35,27,32,30,17,9,33,48,24,6)(2,5,36,45,21,7,4)(8,25,19)(10,34,26,29,31,28,18)"),
    ("R U R' U'", "uulufuufblllllllffdfufffrrubrurrbrrbbbbbddrddddd", "(1,27,35,33,9,3)(2,21,5)(8,30,25,43,19,24)(26,34,28)"),
    ("F L D", "buubublllllllbbruffufdddurrurlffbbdbdurrfffdrddr", "(1,17,19,32,40)(4,20,18,21,31,39,15,12,10,13,7,28,45,47,44,37)(6,25,48,46,35)(8,38,14,9,11)(16,30)(22,43)(23,42)(24,41)"),
    ("U R2 D' F2 L2", "dudddduurffrlrfflbbbflrrlrrlrfbbflbfbllbuuuududd", "(1,8,43,3,46,48)(2,7,44,47,45,5,42,4)(6,41)(9,25,24,33,40,32)(10,34,18,15,39,31,26,23)(11,22)(12,13,28,29)(14,38,35,19,30,27)(16,17)(20,21,36,37)"),
    ("B' D2 L F U' R B2 D", "bdblffbflbdururlrdrbdulruuulrbfdrfddllubffdurlbf", "(1,14,9,46,35,40)(2,39,7,26,15,28,45,36,20,42)(3,27,33)(4,12)(5,44,21,31,29,13,23,34,47,18)(6,22,48,43,11,16,38,30,17,41,32,24)(8,25,19)(10,37)"),
    ("L L L L", "uuuuuuuullllllllffffffffrrrrrrrrbbbbbbbbdddddddd", "()"),
    ("R L R' L'", "uuuuuuuullllllllffffffffrrrrrrrrbbbbbbbbdddddddd", "()"),
];
