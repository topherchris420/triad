//! Standalone SVG plots authored by the Rust experiment; no application UI.
use crate::{
    experiment::EvaluationRow,
    inverse::Dictionary,
    observability::{position_sensitivity, Ambiguity},
};
use anyhow::Result;
use serde_json::Value;
use std::path::Path;

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
fn begin(title: &str, subtitle: &str) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="960" height="680" viewBox="0 0 960 680"><rect width="960" height="680" fill="#fafafa"/><g font-family="Arial,sans-serif" fill="#17202a"><text x="54" y="40" font-size="24">{}</text><text x="54" y="67" font-size="14">{}</text><text x="54" y="655" font-size="12">Vers3Dynamics | TRIAD | idealized 2D simulation | no commercial comparison</text>"##,
        escape(title),
        escape(subtitle)
    )
}
fn color(v: f64, max: f64) -> String {
    let f = (v / max.max(1e-15)).clamp(0.0, 1.0);
    format!(
        "#{:02x}{:02x}{:02x}",
        (245.0 - 170.0 * f) as u8,
        (247.0 - 140.0 * f) as u8,
        (250.0 - 60.0 * f) as u8
    )
}
fn heatmap(
    path: &Path,
    title: &str,
    subtitle: &str,
    xs: &[f64],
    ys: &[f64],
    values: &[Option<f64>],
    units: &str,
) -> Result<()> {
    let max = values.iter().flatten().copied().fold(0.0, f64::max);
    let mut svg = begin(title, subtitle);
    let w = 780.0 / xs.len() as f64;
    let h = 450.0 / ys.len() as f64;
    for (i, x) in xs.iter().enumerate() {
        for (j, y) in ys.iter().enumerate() {
            let value = values[i * ys.len() + j];
            let xx = 100.0 + i as f64 * w;
            let yy = 108.0 + (ys.len() - 1 - j) as f64 * h;
            let fill = value.map(|v| color(v, max)).unwrap_or("#eeeeee".into());
            svg.push_str(&format!(
                r#"<rect x="{xx}" y="{yy}" width="{w}" height="{h}" fill="{fill}" stroke="white"/>"#
            ));
            let label = value.map(|v| format!("{v:.2}")).unwrap_or("n/a".into());
            svg.push_str(&format!(
                r#"<text x="{}" y="{}" font-size="13" text-anchor="middle">{label}</text>"#,
                xx + w / 2.0,
                yy + h / 2.0 + 5.0
            ));
            if i == 0 {
                svg.push_str(&format!(
                    r#"<text x="88" y="{}" font-size="13" text-anchor="end">{:.0}</text>"#,
                    yy + h / 2.0 + 5.0,
                    y * 1000.0
                ));
            }
            if j == 0 {
                svg.push_str(&format!(
                    r#"<text x="{}" y="582" font-size="13" text-anchor="middle">{:.0}</text>"#,
                    xx + w / 2.0,
                    x * 1000.0
                ));
            }
        }
    }
    svg.push_str(&format!(r#"<text x="460" y="610" text-anchor="middle" font-size="15">x (mm)</text><text x="30" y="340" transform="rotate(-90 30 340)" font-size="15">y (mm)</text><text x="100" y="630" font-size="13">{}; range 0 to {:.3}. Values printed per cell.</text></g></svg>"#,escape(units),max));
    std::fs::write(path, svg)?;
    Ok(())
}
pub fn generate(
    root: &Path,
    d: &Dictionary,
    rows: &[EvaluationRow],
    ambiguity: &[Ambiguity],
    summary: &Value,
) -> Result<()> {
    let dir = root.join("results/plots");
    std::fs::create_dir_all(&dir)?;
    let xs = &d.axes[0];
    let ys = &d.axes[1];
    for scenario in ["clean", "strong_drift", "wide_dielectric"] {
        let mut cells = vec![Vec::new(); xs.len() * ys.len()];
        for r in rows.iter().filter(|r| r.scenario == scenario) {
            let ix = (0..xs.len())
                .min_by(|a, b| {
                    (xs[*a] - r.truth[0])
                        .abs()
                        .total_cmp(&(xs[*b] - r.truth[0]).abs())
                })
                .unwrap();
            let iy = (0..ys.len())
                .min_by(|a, b| {
                    (ys[*a] - r.truth[1])
                        .abs()
                        .total_cmp(&(ys[*b] - r.truth[1]).abs())
                })
                .unwrap();
            cells[ix * ys.len() + iy].push(r.error_mm);
        }
        let values: Vec<_> = cells
            .iter()
            .map(|c| {
                if c.is_empty() {
                    None
                } else {
                    Some(c.iter().sum::<f64>() / c.len() as f64)
                }
            })
            .collect();
        heatmap(&dir.join(format!("localization-error-{scenario}.svg")),&format!("Localization error: {scenario}"),
                "Always-return GLS; off-grid refined FEM observations; cells average sampled test errors",xs,ys,&values,"Mean Euclidean error (mm)")?;
    }
    let mut values = Vec::new();
    let mut sensitivity = Vec::new();
    for x in xs {
        for y in ys {
            values.push(Some(
                ambiguity
                    .iter()
                    .filter(|a| a.state[0] == *x && a.state[1] == *y)
                    .map(|a| a.distance_noise_units)
                    .fold(f64::INFINITY, f64::min),
            ));
            sensitivity.push(Some(position_sensitivity(d, [*x, *y, 5.0])));
        }
    }
    heatmap(&dir.join("ambiguity.svg"),"Measurement ambiguity","Nearest alternative >5 mm away; minimum over target material; finite dictionary diagnostic",xs,ys,&values,"Mahalanobis channel distance (smaller means harder to distinguish)")?;
    heatmap(&dir.join("sensitivity.svg"),"Initial position sensitivity","Interpolated Jacobian norm at relative permittivity 5; not nuisance-adjusted observability",xs,ys,&sensitivity,"Frobenius norm (pF/m per mm)")?;
    let mut svg=begin("Baseline scenario comparison","Mean / median / p90 localization error; 100% acceptance; fixed design, independent drift sequences");
    svg.push_str(r#"<text x="80" y="108" font-size="14">Scenario</text><text x="430" y="108" font-size="14">Mean mm</text><text x="570" y="108" font-size="14">Median mm</text><text x="700" y="108" font-size="14">p90 mm</text><text x="810" y="108" font-size="14">Error &gt;5 mm</text>"#);
    for (i, (name, m)) in summary["scenarios"].as_object().unwrap().iter().enumerate() {
        let y = 145 + i * 43;
        svg.push_str(&format!(r#"<text x="80" y="{y}" font-size="15">{name}</text><text x="430" y="{y}" font-size="15">{:.2}</text><text x="570" y="{y}" font-size="15">{:.2}</text><text x="700" y="{y}" font-size="15">{:.2}</text><text x="810" y="{y}" font-size="15">{:.1}%</text>"#,m["mean_mm"].as_f64().unwrap(),m["median_mm"].as_f64().unwrap(),m["p90_mm"].as_f64().unwrap(),100.0*m["over_5mm_rate"].as_f64().unwrap()));
    }
    svg.push_str("</g></svg>");
    std::fs::write(dir.join("scenario-comparison.svg"), svg)?;
    Ok(())
}
