# Physical model and measurement definition

Institution Vers3Dynamics; principal investigator Christopher Woodyard.
Maintained implementation: Rust, Fenris 0.0.33 FEM assembly, Spade 2.15.1
constrained Delaunay triangulation, faer 0.24.4 sparse LU. Dependencies are locked
in Cargo.lock. Fenris authors explicitly describe its API as unstable and warn
against production use; this is a research prototype with numerical validation.
[Fenris documentation](https://docs.rs/fenris/0.0.33/fenris/).

## Geometry and interpretation

All coordinates are meters. The finite rectangular domain is x in [-0.090,0.090]
and y in [0,0.090]. Four zero-thickness conductive boundary strips sit at y=0,
centered at x=-0.027,-0.009,+0.009,+0.027, each 0.008 wide. This is an idealized
cross-section, invariant along the omitted z dimension. The target is a circular
dielectric disk of radius 0.004, representing a uniform infinitely long cylinder
in z. Relative background permittivity is 1; relative target permittivity is
unknown, normally in [2,8]. Target position is searched over x in [-0.024,0.024],
y in [0.008,0.028]. Target and electrodes remain separated.

This is not a realistic finite 3D coplanar sensor. There are no finite electrode
lengths, substrates, circuit parasitics, conductive targets, dielectric losses,
frequency dependence, or nonlinear readout electronics. The grounded enclosure
is part of the defined physical model, not an asserted converged open boundary.

## PDE, excitations and charge

Solve div(epsilon grad(phi))=0 with epsilon=epsilon0*epsilon_r,
epsilon0=8.8541878128e-12 F/m. Excitation j sets electrode j to 1 V and the other
three electrodes plus top and sides to 0 V. The remaining bottom gaps have
homogeneous natural Neumann conditions (zero normal dielectric displacement).
Four independent excitations are solved. The union of grounded outer boundaries
is a fifth conductor. Its unit excitation follows phi_ground=1-sum_j phi_j.

The outward normal points from the dielectric toward conductor. Conductor charge
per length is Q_i=integral_boundary_i epsilon*grad(phi) dot n ds. This convention
gives positive driven self-charge and negative mutual charges. The potential
energy per length is (1/2) integral epsilon*|grad(phi)|^2 dA. The Maxwell matrix
C_ij=integral epsilon*grad(phi_i) dot grad(phi_j) dA is symmetric, positive
semidefinite with a gauge null mode, and its full five-conductor columns sum to
zero. Reduced four-electrode C excludes ground and need not have zero column
sums; charge conservation must include ground.

Finite element stiffness is assembled by Fenris's Laplace operator, multiplying
each element's library-computed matrix by its relative dielectric coefficient.
Dirichlet elimination is exact; faer solves all four RHS with one sparse LU per
matrix. Charges are weak boundary nodal reactions K*phi summed by conductor.
They are compared with the independently accumulated energy projection U^T*K*U.
Symmetry and charge checks primarily test assembly/elimination consistency;
their algebraic nature does not replace mesh convergence or an analytic test.

## Mesh and signals

The mesh conforms to a polygonal approximation of the circular dielectric
interface. Polygon edges have nominal length <=mesh spacing and at least 24
segments. Dielectric coefficients are constant within each conforming triangle;
Fenris's degree-2 quadrature exactly integrates the constant P1-gradient product.
Electrode endpoints are explicitly included. Spacing is uniform in the central
+/-40 by 40 mm region, grading by a factor 1.2 toward remote boundaries.
Production dictionary spacing is 1 mm, observation spacing 0.5 mm, and
validation includes 0.25 mm. Electrode-edge singularities converge slowly.

Ten nonredundant upper-triangle entries of the four-electrode block are used:
C00,C01,C02,C03,C11,C12,C13,C22,C23,C33. They are independent schedule channels,
not a guarantee of ten independent physical degrees of information. Units are
**pF per meter** of cylinder/electrode length (not pF). Off-diagonal channels
retain their signed Maxwell coefficients. With 1 V excitations, numeric charge
in pC/m equals capacitance in pF/m.

The signal f(theta) is target C minus homogeneous C, both on the same target
mesh. Using a matching numerical reference reduces mesh-dependent baseline
errors; physical empty-domain capacitance is position-independent in the
continuum. For target permittivity equal to background, the uniform branch uses
the empty mesh and gives exact zero signal. Radius-mismatch observations use
4.4 mm while the estimator assumes 4 mm.

## Observation equation and information budget

z_st = diag(1+g_st) f(x_st,y_st,epsilon_st) + b_s + d_st + a_st*1 + eta_st.
All additive terms are pF/m. Gaussian eta has covariance sigma^2[(1-rho)I+rho11T]
with sigma=0.003 pF/m and rho=0.35. The fixed per-sequence offsets b_s have
standard deviation 0.01. Empty-target calibration consists of 24 samples b_s+eta
before sequence drift begins. Inference subtracts their mean; the declared GLS
covariance includes sigma^2/24 calibration-mean error.

Mild gain, channel offset and shared drift are independent random walks with
per-frame increments 0.004 (dimensionless), 0.0004 pF/m and 0.0007 pF/m;
strong/combined drift multiplies them by 4. Correlated disturbance uses a shared
random walk of 0.002 pF/m per frame. Channel disturbance ramps one channel to
0.025 pF/m across 12 frames. No measurement clipping is imposed. Gain acts on
the baseline-subtracted target signal: this simplifies real gain-on-total-C
effects and limits physical interpretation.

Calibration estimates b_s but cannot identify future gain/offset/shared drift.
Even unlimited empty observations cannot determine a multiplicative gain on
zero target signal without a known nonzero reference excitation. Unknown
material, position and unrestricted per-channel offsets are not identifiable
from one ten-channel vector: arbitrary offsets can explain any candidate state.
The conventional estimator assumes residual drift zero and estimates x,y,epsilon
jointly; drift scenarios deliberately violate that assumption. It never receives
true material, position, gain, drift, scenario label or test outcomes as inputs.

## Validation boundary

`results/validation.json` records actual mesh, domain, conservation, reciprocity,
reaction/energy, sign, equality and analytic affine/layered-dielectric results.
The baseline command refuses a failed or configuration-mismatched validation.
The unit suite independently checks PSD/gauge structure and an off-grid mesh
probe. All numerical error and residual ratios must be interpreted with their
units. Passing finite relative tolerances establishes an adequate prototype
discretization for these probes; it does not make numerical errors disappear
relative to small noise or prove all target configurations are resolved.
