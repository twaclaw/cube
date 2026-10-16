"""Turn the SageMath dump into the Rust table the tests read."""
import json, sys

source, target = sys.argv[1], sys.argv[2]
cases = json.load(sys.stdin if source == "-" else open(source))["cases"]
rows = "\n".join(
    "    (%s, %s)," % (json.dumps(" ".join(c["moves"])), json.dumps(c["state"]))
    for c in cases
)
open(target, "w").write(
    '''//! Ground truth taken from SageMath's `CubeGroup`, which uses the same facelet
//! numbering as the notebooks: faces in the order U, L, F, R, B, D, row major,
//! centers left out, so 1..=48 on a 3x3.
//!
//! Regenerate with `make vectors`.

/// `(maneuver, facelet colors after applying it to a solved cube)`
pub const CASES: &[(&str, &str)] = &[
%s
];
''' % rows
)
print(f"{target}: {len(cases)} cases")
