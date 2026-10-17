"""Turn the SageMath dump into the Rust table the tests read."""

import json
import sys

HEADER = """\
//! Ground truth taken from SageMath's `CubeGroup`, which uses the same facelet
//! numbering as the notebooks: faces in the order U, L, F, R, B, D, row major,
//! centers left out, so 1..=48 on a 3x3.
//!
//! Regenerate with `make vectors`.

/// `(maneuver, facelet colors after applying it, the same state as a permutation)`
// One case per line reads better than what rustfmt would do with it.
#[rustfmt::skip]
pub const CASES: &[(&str, &str, &str)] = &[
"""


def row(case):
    """One case as a Rust tuple."""
    moves = json.dumps(" ".join(case["moves"]))
    state = json.dumps(case["state"])
    perm = json.dumps(case["perm"])
    return f"    ({moves}, {state}, {perm}),"


def read_cases(source):
    """The dump, from a file or from standard input."""
    if source == "-":
        return json.load(sys.stdin)["cases"]
    with open(source, encoding="utf-8") as dump:
        return json.load(dump)["cases"]


def main():
    source, target = sys.argv[1], sys.argv[2]
    cases = read_cases(source)
    rows = "\n".join(row(case) for case in cases)
    with open(target, "w", encoding="utf-8") as table:
        table.write(f"{HEADER}{rows}\n];\n")
    print(f"{target}: {len(cases)} cases")


if __name__ == "__main__":
    main()
