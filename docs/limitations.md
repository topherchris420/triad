# Limitations and research boundary

1. **Idealized physics:** lossless static 2D dielectric cylinder, zero-thickness
   bottom strips, insulating gaps, grounded sides/top. No finite 3D target,
   electronics, substrate, parasitics or hardware measurements. Grounded-domain
   sensitivity is assessed for one larger enclosure, not the infinite-domain limit.
2. **Numerical approximation:** target polygons and linear elements; interface,
   electrode-edge and far-boundary errors remain. Production/coarse vs refined
   observations prevent a same-mesh inverse crime but share Fenris and geometry;
   they are not independent solver validation. Relative mesh gates do not ensure
   discretization error is below the measurement noise. Reports expose both.
3. **Dictionary interpolation:** 6 mm x and 4 mm y spacing and five material
   levels form a tractable coarse surrogate. Multilinear interpolation can bias
   inverse predictions even when FEM mesh convergence passes. Its error is a
   separate numerical approximation; fine observations honestly include it.
4. **Inverse search:** five separated multistarts with bounded damped Gauss–Newton
   are not a guarantee of global optimization or unique inversion. Failed
   convergence is recorded; the baseline returns the best available fit. Unknown
   permittivity is bounded [2,8]; wider test material produces deliberate mismatch.
5. **Ambiguity:** minimum pairwise noise-weighted dictionary distances at positions
   separated >5 mm, over material candidates. Nearby alternatives not represented
   by the grid may be missed. "Global ambiguity" scenario selects relatively
   difficult dictionary states; the name does not establish exact nonuniqueness.
   Background-equal targets have exact zero signal everywhere and are explicitly
   nonlocalizable. Position-only Jacobian norm is not nuisance-adjusted information.
6. **Nuisance limits:** empty histories estimate initial offsets, not future drift
   or multiplicative gain. Unrestricted per-channel offsets make single-frame
   localization unidentifiable. This estimator sets future nuisance drift to zero
   and is deliberately tested under that misspecification. No true drift, material,
   position, scenario label or test outcome is provided as an estimator input.
7. **Measurement simplification:** multiplicative drift acts on target-induced
   capacitance, rather than full capacitance. Scalar sigma and rho are designed
   simulation settings, not measured hardware noise. Random target paths are not
   a motion prior; no temporal tracking estimator is used.
8. **Statistical scope:** three seeds, nine test sequences per scenario and fixed
   physical pools. Whole-sequence intervals condition on those pools and may be
   imprecise. Training/calibration/test share the fixed spatial design but have
   independent observations and histories. Drift sequences are not split across
   partitions. This is a pilot, not a definitive distribution-wide result.
9. **Calibration/selection:** 100% acceptance only. Posterior regions, C1/C2, local
   nuisance-adjusted observability, method D, matched-coverage selective risk and
   deployable threshold calibration remain unimplemented. The original hypothesis
   cannot be accepted or rejected from the baseline results.
10. **Commercial and novelty:** source mechanisms differ from this experiment.
    No executable/hardware CAPSENSE comparison is available, no proprietary binary
    was analyzed, and no commercial superiority or defect claim is supported.
    User-reported Rust permission is documented separately from the published
    license. Established physics, GLS, baseline calibration and ambiguity searches
    alone are not novel contributions.
11. **Engineering:** Fenris has unstable research APIs and transitive legacy
    parsers with compiler future-compatibility notices. Cargo.lock and tests pin
    this run. SVG plots are scientific artifacts, not a dashboard. Memory/runtime
    are host-specific; Windows peak working set is measured at process level.

Next justified step after these milestones is a focused surrogate/information
audit, then the predeclared strong-posterior comparison with new test sequences.
3D simulation, hardware, electrode co-design and manuscript preparation require
evidence beyond this pilot and should not be undertaken automatically.
