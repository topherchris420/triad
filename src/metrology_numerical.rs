//! Independent numerical development probes, never confirmatory test labels.
use crate::{
    adaptive::distance,
    forward::{ElectrodeLayout, Model},
    research_integrity::write_new_json,
};
use anyhow::{ensure, Result};
use serde_json::json;
use std::{path::Path, time::Instant};

pub const PROBES: [[f64; 3]; 4] = [
    [-0.0267, 0.0063, 7.7],
    [-0.0127, 0.0131, 3.1],
    [0.0029, 0.0223, 5.7],
    [0.0203, 0.0307, 2.3],
];
pub fn validate_protocol(p: &serde_json::Value) -> Result<()> {
    let expected = json!({"version":"5.0-numerical-development","status":"frozen_before_execution_not_confirmatory","probes":PROBES,"spacings_m":[0.001,0.0005,0.00025],"layouts":["baseline","wider"],"expanded_domain_m":[0.12,0.12],"background_drift":1.03,"edge_interface_candidate":"0.5mm mesh; quarter-spacing electrode-edge nodes; double interface polygon resolution","mesh_inspection_limit_whitened":1.0,"solver_residual_limit":1e-9,"noise":"unchanged measurements::whiten, 24 calibration frames","failure_policy":"write per-probe evidence; no readiness claim from mesh agreement"});
    ensure!(
        p == &expected,
        "numerical protocol is incompatible with implementation; version a new study"
    );
    Ok(())
}
pub fn run(output: &Path) -> Result<()> {
    let protocol = include_bytes!("../experiments/metrology-numerical-v5.json");
    validate_protocol(&serde_json::from_slice(protocol)?)?;
    std::fs::create_dir(output)?;
    write_new_json(
        &output.join("started.json"),
        &json!({"protocol_sha256":crate::evaluation::hash(protocol),"status":"started; summary required for completion"}),
    )?;
    let mut rows = Vec::new();
    for (i, state) in PROBES.into_iter().enumerate() {
        for (name, layout) in [
            ("baseline", ElectrodeLayout::default()),
            (
                "wider",
                ElectrodeLayout {
                    centers: [-0.030, -0.010, 0.010, 0.030],
                    half_width: 0.004,
                },
            ),
        ] {
            let start = Instant::now();
            let mut solutions = Vec::new();
            for spacing in [0.001, 0.0005, 0.00025] {
                solutions.push(Model::new(spacing).solve_with_layout(Some(state), layout)?);
            }
            let mut boundary = Model::new(0.0005);
            boundary.geometry.half_width = 0.12;
            boundary.geometry.height = 0.12;
            let expanded = boundary.solve_with_layout(Some(state), layout)?;
            let local_refined = Model::new(0.0005).solve_refined(Some(state), layout, true)?;
            let mut drift = Model::new(0.0005);
            drift.geometry.background = 1.03;
            let drifted = drift.solve_with_layout(Some(state), layout)?;
            let row = json!({"state":state,"layout":name,"spacing_m":[0.001,0.0005,0.00025],"coarse_medium":distance(&solutions[0].signal,&solutions[1].signal),"medium_fine":distance(&solutions[1].signal,&solutions[2].signal),"boundary_sensitivity":distance(&solutions[1].signal,&expanded.signal),"background_drift_sensitivity":distance(&solutions[1].signal,&drifted.signal),"solutions":solutions,"elapsed_seconds":start.elapsed().as_secs_f64()});
            let mut row = row;
            row["edge_interface_candidate_vs_fine"] =
                json!(distance(&local_refined.signal, &solutions[2].signal));
            row["edge_interface_candidate"] = json!(local_refined);
            write_new_json(&output.join(format!("probe-{i}-{name}.json")), &row)?;
            rows.push(row);
        }
    }
    let mesh = rows
        .iter()
        .all(|r| r["medium_fine"].as_f64().unwrap() <= 1.0);
    let solver = rows.iter().all(|r| {
        r["solutions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["free_residual"].as_f64().unwrap() < 1e-9)
    });
    write_new_json(
        &output.join("summary.json"),
        &json!({"status":"exploratory_numerical_study","protocol_sha256":crate::evaluation::hash(protocol),"rows":rows,"mesh_inspection_pass":mesh,"solver_inspection_pass":solver,"numerical_readiness":false,"missing":["validated continuum error bound","independent interpolation and observability acceptance evidence","3D and hardware structural validation"],"interpretation":"Three resolutions measure disagreement, not distance to physical truth. Drift is a model sensitivity, not discretization error; original noise unchanged."}),
    )?;
    Ok(())
}
