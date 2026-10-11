use crate::{
    evaluation::hash,
    forward::{dirichlet, multiply, stiffness, Geometry, Model, PF},
};
use anyhow::{ensure, Result};
use fenris::{connectivity::Tri3d2Connectivity, mesh::TriangleMesh2d, nalgebra::Point2};
use serde_json::{json, Value};

fn norm(v: impl Iterator<Item = f64>) -> f64 {
    v.map(|x| x * x).sum::<f64>().sqrt()
}
fn relative(a: &[f64], b: &[f64]) -> f64 {
    norm(a.iter().zip(b).map(|(x, y)| x - y)) / norm(b.iter().copied()).max(1e-30)
}
fn flat(a: &[[f64; 5]; 5]) -> Vec<f64> {
    a.iter().flatten().copied().collect()
}
pub fn benchmarks() -> Result<Value> {
    let n = 20;
    let mut v = Vec::new();
    let mut triangles = Vec::new();
    for i in 0..=n {
        for j in 0..=n {
            v.push(Point2::new(i as f64 / n as f64, j as f64 / n as f64));
        }
    }
    for i in 0..n {
        for j in 0..n {
            let a = i * (n + 1) + j;
            let b = a + n + 1;
            triangles.push(Tri3d2Connectivity([a, b, b + 1]));
            triangles.push(Tri3d2Connectivity([a, b + 1, a + 1]));
        }
    }
    let mesh = TriangleMesh2d::from_vertices_and_connectivity(v, triangles);
    let k = stiffness(&mesh, vec![1.0; mesh.connectivity().len()])?;
    let boundary: Vec<_> = mesh
        .vertices()
        .iter()
        .map(|p| {
            if p.x < 1e-10 || p.y < 1e-10 || p.x > 1.0 - 1e-10 || p.y > 1.0 - 1e-10 {
                Some(vec![p.x + 2.0 * p.y])
            } else {
                None
            }
        })
        .collect();
    let u = dirichlet(&k, &boundary, 1)?;
    let affine = mesh
        .vertices()
        .iter()
        .zip(&u)
        .map(|(p, u)| (u[0] - p.x - 2.0 * p.y).abs())
        .fold(0.0, f64::max);
    let coefficients = mesh
        .connectivity()
        .iter()
        .map(|t| {
            if t.0.iter().map(|i| mesh.vertices()[*i].y).sum::<f64>() / 3.0 < 0.5 {
                2.0
            } else {
                6.0
            }
        })
        .collect();
    let k = stiffness(&mesh, coefficients)?;
    let boundary: Vec<_> = mesh
        .vertices()
        .iter()
        .map(|p| {
            if p.y < 1e-10 {
                Some(vec![0.0])
            } else if p.y > 1.0 - 1e-10 {
                Some(vec![1.0])
            } else {
                None
            }
        })
        .collect();
    let u = dirichlet(&k, &boundary, 1)?;
    let r = multiply(&k, &u);
    let numeric = PF * u.iter().zip(r).map(|(u, r)| u[0] * r[0]).sum::<f64>();
    let analytic = PF / (0.5 / 2.0 + 0.5 / 6.0);
    Ok(
        json!({"affine_max_error":affine,"layered_capacitance_pf_per_m":numeric,
        "layered_analytic_pf_per_m":analytic,"layered_relative_error":(numeric/analytic-1.0).abs()}),
    )
}

pub fn run(root: &std::path::Path) -> Result<Value> {
    let bytes = std::fs::read(root.join("experiments/config.json"))?;
    let cfg: Value = serde_json::from_slice(&bytes)?;
    let limits = &cfg["validation"];
    let probes = [
        [0.0, 0.016, 5.0],
        [-0.018, 0.008, 8.0],
        [0.012, 0.028, 2.0],
        [0.024, 0.020, 6.5],
    ];
    let benchmark = benchmarks()?;
    let mut mesh_probes = Vec::new();
    let mut domain_probes = Vec::new();
    let mut charge: f64 = 0.0;
    let mut symmetry: f64 = 0.0;
    let mut consistency: f64 = 0.0;
    let mut residual: f64 = 0.0;
    let mut signs = true;
    for state in probes {
        println!("Validate target {state:?}");
        let a = Model::new(0.001).solve(Some(state))?;
        let b = Model::new(0.0005).solve(Some(state))?;
        let c = Model::new(0.00025).solve(Some(state))?;
        for s in [&a, &b, &c] {
            charge = charge.max(s.conservation);
            residual = residual.max(s.free_residual);
            let transpose: Vec<_> = (0..5)
                .flat_map(|i| (0..5).map(move |j| s.capacitance[j][i]))
                .collect();
            symmetry = symmetry.max(relative(&flat(&s.capacitance), &transpose));
            consistency = consistency.max(relative(
                &flat(&s.capacitance),
                &flat(&s.reaction_capacitance),
            ));
            for i in 0..5 {
                signs &= s.capacitance[i][i] > 0.0;
                for j in 0..5 {
                    if i != j {
                        signs &= s.capacitance[i][j] < 1e-9;
                    }
                }
            }
        }
        mesh_probes.push(json!({"state":state,"total_1mm_to_half_relative":relative(&flat(&a.capacitance),&flat(&b.capacitance)),
            "signal_1mm_to_half_relative":relative(&a.signal,&b.signal),"signal_half_to_quarter_relative":relative(&b.signal,&c.signal),
            "signal_1mm_to_half_noise_units":norm(a.signal.iter().zip(b.signal).map(|(x,y)|x-y))/0.003,
            "signal_norm_pf_per_m":norm(c.signal.into_iter()),"nodes_1mm_half_quarter":[a.nodes,b.nodes,c.nodes]}));
        let extended = Model {
            spacing: 0.0005,
            geometry: Geometry {
                half_width: 0.120,
                height: 0.120,
                ..Geometry::default()
            },
        }
        .solve(Some(state))?;
        domain_probes.push(
            json!({"state":state,"relative_signal_change":relative(&b.signal,&extended.signal)}),
        );
    }
    let model = Model::new(0.001);
    let uniform = model.solve(Some([0.007, 0.019, 1.0]))?;
    let empty = model.solve(None)?;
    let limit = |name: &str| limits[name].as_f64().unwrap();
    let max = |name: &str| {
        mesh_probes
            .iter()
            .map(|p| p[name].as_f64().unwrap())
            .fold(0.0, f64::max)
    };
    let checks = json!({
        "mesh_total":max("total_1mm_to_half_relative")<=limit("mesh_total_relative_limit"),
        "mesh_signal":max("signal_1mm_to_half_relative")<=limit("mesh_signal_relative_limit"),
        "fine_mesh_signal":max("signal_half_to_quarter_relative")<=limit("fine_mesh_signal_relative_limit"),
        "charge_conservation":charge<=limit("charge_relative_limit"),
        "reciprocity":symmetry<=limit("reciprocity_relative_limit"),
        "reaction_energy":consistency<=limit("reaction_energy_relative_limit"),
        "free_residual":residual<=limit("charge_relative_limit"),
        "domain_sensitivity":domain_probes.iter().map(|p|p["relative_signal_change"].as_f64().unwrap()).fold(0.0,f64::max)<=limit("domain_signal_relative_limit"),
        "analytical_benchmarks":benchmark["affine_max_error"].as_f64().unwrap()<=limit("benchmark_relative_limit") && benchmark["layered_relative_error"].as_f64().unwrap()<=limit("benchmark_relative_limit"),
        "background_equality":uniform.vector==empty.vector && uniform.signal==[0.0;10],"physical_signs":signs,
        "measurement_consistency":consistency<=limit("reaction_energy_relative_limit")});
    let passed = checks
        .as_object()
        .unwrap()
        .values()
        .all(|v| v.as_bool() == Some(true));
    let report = json!({"passed":passed,"checks":checks,"config_sha256":hash(&bytes),"benchmarks":benchmark,
        "mesh_probes":mesh_probes,"domain_probes":domain_probes,"maximum_charge_relative":charge,
        "maximum_reciprocity_relative":symmetry,"maximum_reaction_energy_relative":consistency,"maximum_free_residual_relative":residual});
    std::fs::create_dir_all(root.join("results"))?;
    std::fs::write(
        root.join("results/validation.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    ensure!(passed, "Forward validation failed; localization is blocked");
    Ok(report)
}
