//! P1 electrostatics assembled by Fenris; Spade constrained triangulation.
//! SI coordinates; charges/capacitance per omitted cylinder length in pF/m.
use anyhow::{bail, Result};
use faer::{
    prelude::*,
    sparse::{SparseColMat, Triplet},
};
use fenris::{
    assembly::{
        global::CsrAssembler,
        local::{
            ElementConnectivityAssembler, ElementEllipticAssemblerBuilder, ElementMatrixAssembler,
            UniformQuadratureTable,
        },
        operators::LaplaceOperator,
    },
    connectivity::Tri3d2Connectivity,
    mesh::TriangleMesh2d,
    nalgebra::{DMatrixViewMut, DVector, Point2},
    nalgebra_sparse::CsrMatrix,
    quadrature,
};
use serde::{Deserialize, Serialize};
use spade::{ConstrainedDelaunayTriangulation, Point2 as SpadePoint, Triangulation};

pub const PF: f64 = 8.8541878128;
pub const CHANNELS: [(usize, usize); 10] = [
    (0, 0),
    (0, 1),
    (0, 2),
    (0, 3),
    (1, 1),
    (1, 2),
    (1, 3),
    (2, 2),
    (2, 3),
    (3, 3),
];

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Geometry {
    pub half_width: f64,
    pub height: f64,
    pub radius: f64,
    pub background: f64,
}
impl Default for Geometry {
    fn default() -> Self {
        Self {
            half_width: 0.090,
            height: 0.090,
            radius: 0.004,
            background: 1.0,
        }
    }
}

/// Four independently addressable equal-width electrodes, all on y=0.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct ElectrodeLayout {
    pub centers: [f64; 4],
    pub half_width: f64,
}
impl Default for ElectrodeLayout {
    fn default() -> Self {
        Self { centers: [-0.027, -0.009, 0.009, 0.027], half_width: 0.004 }
    }
}
impl ElectrodeLayout {
    pub fn validate(self, g: Geometry) -> Result<()> {
        if !self.half_width.is_finite() || self.half_width <= 0.0 {
            bail!("invalid electrode half width");
        }
        for (i, center) in self.centers.iter().enumerate() {
            if !center.is_finite() || center.abs() + self.half_width >= g.half_width {
                bail!("electrode outside domain");
            }
            if i > 0 && *center - self.centers[i-1] <= 2.0 * self.half_width {
                bail!("electrode strips overlap or touch");
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Solution {
    pub capacitance: [[f64; 5]; 5],
    pub reaction_capacitance: [[f64; 5]; 5],
    pub vector: [f64; 10],
    pub signal: [f64; 10],
    pub nodes: usize,
    pub elements: usize,
    pub free_residual: f64,
    pub conservation: f64,
}

struct WeightedAssembler<A> {
    inner: A,
    coefficients: Vec<f64>,
}
impl<A: ElementConnectivityAssembler> ElementConnectivityAssembler for WeightedAssembler<A> {
    fn solution_dim(&self) -> usize {
        self.inner.solution_dim()
    }
    fn num_elements(&self) -> usize {
        self.inner.num_elements()
    }
    fn num_nodes(&self) -> usize {
        self.inner.num_nodes()
    }
    fn element_node_count(&self, i: usize) -> usize {
        self.inner.element_node_count(i)
    }
    fn populate_element_nodes(&self, out: &mut [usize], i: usize) {
        self.inner.populate_element_nodes(out, i)
    }
}
impl<A: ElementMatrixAssembler<f64>> ElementMatrixAssembler<f64> for WeightedAssembler<A> {
    fn assemble_element_matrix_into(
        &self,
        i: usize,
        mut out: DMatrixViewMut<f64>,
    ) -> fenris::eyre::Result<()> {
        self.inner
            .assemble_element_matrix_into(i, out.rows_mut(0, 3))?;
        out *= self.coefficients[i];
        Ok(())
    }
}

fn inside_polygon(p: [f64; 2], polygon: &[[f64; 2]]) -> bool {
    let mut inside = false;
    for i in 0..polygon.len() {
        let a = polygon[i];
        let b = polygon[(i + 1) % polygon.len()];
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
    }
    inside
}

pub fn mesh(
    spacing: f64,
    g: Geometry,
    position: Option<[f64; 2]>,
) -> Result<(TriangleMesh2d<f64>, Vec<bool>)> {
    mesh_with_layout(spacing, g, position, ElectrodeLayout::default())
}
pub fn mesh_with_layout(
    spacing: f64,
    g: Geometry,
    position: Option<[f64; 2]>,
    layout: ElectrodeLayout,
) -> Result<(TriangleMesh2d<f64>, Vec<bool>)> {
    layout.validate(g)?;
    if spacing <= 0.0 || g.background <= 0.0 || g.radius <= 0.0 {
        bail!("invalid geometry");
    }
    let mut tri = ConstrainedDelaunayTriangulation::<SpadePoint<f64>>::new();
    // Refine the sensing region and grade toward remote grounded boundaries.
    let axis = |end: f64| {
        let core = 0.040_f64.min(end);
        let n = (core / spacing).ceil() as usize;
        let mut values: Vec<f64> = (0..=n).map(|i| core * i as f64 / n as f64).collect();
        let mut p = core;
        let mut step = spacing;
        while p < end - 1e-12 {
            step *= 1.2;
            p = (p + step).min(end);
            values.push(p);
        }
        values
    };
    let positive = axis(g.half_width);
    let mut xs: Vec<f64> = positive
        .iter()
        .rev()
        .skip(0)
        .filter(|x| **x > 0.0)
        .map(|x| -x)
        .chain(positive.iter().copied())
        .collect();
    let ys = axis(g.height);
    for center in layout.centers {
        xs.extend([center - layout.half_width, center + layout.half_width]);
    }
    xs.sort_by(f64::total_cmp);
    xs.dedup_by(|a, b| (*a - *b).abs() < 1e-11);
    let mut polygon = Vec::new();
    if let Some(p) = position {
        if p[0].abs() + g.radius >= g.half_width || p[1] <= g.radius || p[1] + g.radius >= g.height
        {
            bail!("target outside domain");
        }
        let n = (2.0 * std::f64::consts::PI * g.radius / spacing)
            .ceil()
            .max(24.0) as usize;
        polygon = (0..n)
            .map(|i| {
                let a = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
                [p[0] + g.radius * a.cos(), p[1] + g.radius * a.sin()]
            })
            .collect();
    }
    for x in xs {
        for y in ys.iter().copied() {
            if let Some(p) = position {
                let d = ((x - p[0]).powi(2) + (y - p[1]).powi(2)).sqrt();
                if (d - g.radius).abs() < spacing * 0.22 {
                    continue;
                }
            }
            tri.insert(SpadePoint::new(x, y))?;
        }
    }
    let ring: Vec<_> = polygon
        .iter()
        .map(|p| tri.insert(SpadePoint::new(p[0], p[1])))
        .collect::<std::result::Result<_, _>>()?;
    for i in 0..ring.len() {
        tri.add_constraint(ring[i], ring[(i + 1) % ring.len()]);
    }
    let vertices: Vec<_> = tri
        .vertices()
        .map(|v| Point2::new(v.position().x, v.position().y))
        .collect();
    let mut connectivity = Vec::new();
    let mut interior = Vec::new();
    for f in tri.inner_faces() {
        let v = f.vertices();
        let indices = [v[0].index(), v[1].index(), v[2].index()];
        let a = vertices[indices[0]];
        let b = vertices[indices[1]];
        let c = vertices[indices[2]];
        let center = [(a.x + b.x + c.x) / 3.0, (a.y + b.y + c.y) / 3.0];
        interior.push(!polygon.is_empty() && inside_polygon(center, &polygon));
        connectivity.push(Tri3d2Connectivity(indices));
    }
    Ok((
        TriangleMesh2d::from_vertices_and_connectivity(vertices, connectivity),
        interior,
    ))
}

pub fn stiffness(mesh: &TriangleMesh2d<f64>, coefficients: Vec<f64>) -> Result<CsrMatrix<f64>> {
    let (weights, points) =
        quadrature::total_order::triangle(2).map_err(|e| anyhow::anyhow!("quadrature: {e}"))?;
    let table = UniformQuadratureTable::from_points_and_weights(points, weights);
    let zero = DVector::<f64>::zeros(mesh.vertices().len());
    let inner = ElementEllipticAssemblerBuilder::new()
        .with_finite_element_space(mesh)
        .with_operator(&LaplaceOperator)
        .with_quadrature_table(&table)
        .with_u(&zero)
        .build();
    CsrAssembler::default()
        .assemble(&WeightedAssembler {
            inner,
            coefficients,
        })
        .map_err(|e| anyhow::anyhow!("Fenris assembly: {e}"))
}

/// Solve arbitrary nodal Dirichlet conditions with sparse LU, library solver.
pub fn dirichlet(
    k: &CsrMatrix<f64>,
    boundary: &[Option<Vec<f64>>],
    columns: usize,
) -> Result<Vec<Vec<f64>>> {
    let n = k.nrows();
    let mut map = vec![usize::MAX; n];
    let mut nf = 0;
    for i in 0..n {
        if boundary[i].is_none() {
            map[i] = nf;
            nf += 1;
        }
    }
    let mut entries = Vec::new();
    let mut rhs = faer::Mat::zeros(nf, columns);
    for (i, j, value) in k.triplet_iter() {
        if map[i] != usize::MAX {
            if map[j] != usize::MAX {
                entries.push(Triplet::new(map[i], map[j], *value));
            } else {
                for c in 0..columns {
                    rhs[(map[i], c)] -= value * boundary[j].as_ref().unwrap()[c];
                }
            }
        }
    }
    let matrix = SparseColMat::<usize, f64>::try_new_from_triplets(nf, nf, &entries)?;
    let lu = matrix.sp_lu()?;
    let solved = lu.solve(&rhs);
    Ok((0..n)
        .map(|i| {
            if let Some(v) = &boundary[i] {
                v.clone()
            } else {
                (0..columns).map(|c| solved[(map[i], c)]).collect()
            }
        })
        .collect())
}

pub fn multiply(k: &CsrMatrix<f64>, u: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let mut r = vec![vec![0.0; u[0].len()]; u.len()];
    for (i, j, v) in k.triplet_iter() {
        for c in 0..u[0].len() {
            r[i][c] += v * u[j][c];
        }
    }
    r
}

fn solve_matrix(
    mesh: &TriangleMesh2d<f64>,
    k: &CsrMatrix<f64>,
    g: Geometry,
    layout: ElectrodeLayout,
) -> Result<Solution> {
    let mut group = vec![None; mesh.vertices().len()];
    for (i, p) in mesh.vertices().iter().enumerate() {
        if (p.x.abs() - g.half_width).abs() < 1e-10 || (p.y - g.height).abs() < 1e-10 {
            group[i] = Some(4);
        } else if p.y.abs() < 1e-10 {
            for (j, c) in layout.centers.iter().enumerate() {
                if (p.x - c).abs() <= layout.half_width + 1e-10 {
                    group[i] = Some(j);
                }
            }
        }
    }
    let boundary: Vec<_> = group
        .iter()
        .map(|g| g.map(|j| (0..4).map(|c| if c == j { 1.0 } else { 0.0 }).collect()))
        .collect();
    let mut u = dirichlet(k, &boundary, 4)?;
    for row in &mut u {
        row.push(1.0 - row.iter().sum::<f64>());
    }
    let r = multiply(k, &u);
    let mut energy = [[0.0; 5]; 5];
    let mut charge = [[0.0; 5]; 5];
    let mut free = 0.0;
    for i in 0..u.len() {
        for a in 0..5 {
            for b in 0..5 {
                energy[a][b] += PF * u[i][a] * r[i][b];
            }
        }
        if let Some(j) = group[i] {
            for b in 0..5 {
                charge[j][b] += PF * r[i][b];
            }
        } else {
            for b in 0..5 {
                free += (PF * r[i][b]).powi(2);
            }
        }
    }
    let norm = energy.iter().flatten().map(|x| x * x).sum::<f64>().sqrt();
    let conservation = (0..5)
        .map(|b| charge.iter().map(|a| a[b]).sum::<f64>().powi(2))
        .sum::<f64>()
        .sqrt()
        / norm;
    let vector = std::array::from_fn(|c| energy[CHANNELS[c].0][CHANNELS[c].1]);
    Ok(Solution {
        capacitance: energy,
        reaction_capacitance: charge,
        vector,
        signal: [0.0; 10],
        nodes: u.len(),
        elements: mesh.connectivity().len(),
        free_residual: free.sqrt() / norm,
        conservation,
    })
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Model {
    pub spacing: f64,
    pub geometry: Geometry,
}
impl Model {
    pub fn new(spacing: f64) -> Self {
        Self {
            spacing,
            geometry: Geometry::default(),
        }
    }
    pub fn solve(&self, state: Option<[f64; 3]>) -> Result<Solution> {
        self.solve_with_layout(state, ElectrodeLayout::default())
    }
    pub fn solve_with_layout(
        &self,
        state: Option<[f64; 3]>,
        layout: ElectrodeLayout,
    ) -> Result<Solution> {
        layout.validate(self.geometry)?;
        if state.is_some_and(|s| !s.iter().all(|v| v.is_finite()) || s[2] <= 0.0) {
            bail!("invalid target state");
        }
        // Uniform permittivity uses exactly the same mesh as the empty target.
        let state = state.filter(|s| s[2] != self.geometry.background);
        let (mesh, inside) = mesh_with_layout(self.spacing, self.geometry, state.map(|s| [s[0], s[1]]), layout)?;
        let coefficients = inside
            .iter()
            .map(|b| {
                if *b {
                    state.unwrap()[2]
                } else {
                    self.geometry.background
                }
            })
            .collect();
        let k = stiffness(&mesh, coefficients)?;
        let mut solution = solve_matrix(&mesh, &k, self.geometry, layout)?;
        if state.is_some() {
            // Same-mesh homogeneous reference cancels mesh-dependent empty offset.
            let empty = solve_matrix(
                &mesh,
                &stiffness(&mesh, vec![self.geometry.background; inside.len()])?,
                self.geometry,
                layout,
            )?;
            solution.signal = std::array::from_fn(|i| solution.vector[i] - empty.vector[i]);
        }
        Ok(solution)
    }
}
