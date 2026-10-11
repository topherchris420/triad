# Protocol amendments before final localization results

The user requested a Rust implementation after the initial Python scaffold was
written. The maintained package is entirely Rust. Python is not needed to build,
validate, generate data, estimate positions, or plot results.

The initial nonconforming dielectric quadrature model failed predetermined mesh
and domain gates (`results/development-validation-python-failed.json`). Its
configuration is retained as `experiments/protocol-v1-development.json`. No
localization performance was generated from that model. Maximum target signal
mesh changes were 10.24% (1 to 0.5 mm) and 6.90% (0.5 to 0.25 mm); domain change
was 22.97%, beyond the 8%, 4%, and 20% gates respectively.

Version 2 replaces cut-element dielectric sampling with target-conforming
constrained triangulation and piecewise constant element permittivity, and moves
the grounded enclosure from +/-60 by 60 mm to +/-90 by 90 mm. Domain validation
compares to +/-120 by 120 mm. Numerical acceptance tolerances are unchanged.
Mesh spacing is uniform in the central +/-40 by 40 mm region and graded outside
that region. Target polygon edges shrink with mesh spacing (minimum 24 edges).
Homogeneous reference and heterogeneous signal are evaluated on the same mesh
to avoid empty-reference discretization bias.

Final observations use an independent refined 0.5 mm mesh, the inverse dictionary
uses 1 mm. To bound CPU cost, fixed continuous off-grid physical pools cover 54
sites, with distinct wide-material, high-target, ambiguous-state, and radius
mismatch pools. The geometry/material design pools are fixed before predictions;
all sequence-specific noise/drift draws are independent. Splits share a fixed
experimental design, not observations, histories, or drift processes. Bootstrap
intervals describe sequence variation conditional on that design; they do not
describe uncertainty over every possible continuous target configuration.

No uncertainty-aware selection method is implemented. The source reference
audit acknowledges user-reported permission for Rust, but TRIAD remains an
independent mathematical experiment and contains no commercial source port.

## Cache serialization correction

The first cached replay revealed differences in the last floating-point bits
because serde_json's default parsing does not guarantee exact float round trips.
Piecewise inverse optimization could change stopping flags from those tiny
differences. The `float_roundtrip` feature is now enabled and a 20,000-value
bitwise round-trip regression test is required. This fixes numerical storage;
it changes no geometry, noise/drift parameter, random seed, loss, optimizer
setting, tolerance or experimental endpoint. First-run artifacts are retained.
Any observed replay difference is reported rather than concealed; the final
repeat verification requires identical dataset and localization outputs on this
host, excluding inherently variable runtime measurements.
