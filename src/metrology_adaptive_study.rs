//! Development comparison; truth is used only to generate data and score outputs.
use crate::{
    adaptive::refine, forward::Model, inverse::Estimator, measurements,
    research_integrity::write_new_json,
};
use anyhow::Result;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde_json::json;
use std::{path::Path, time::Instant};

pub fn run(root: &Path, output: &Path) -> Result<()> {
    std::fs::create_dir(output)?;
    let dictionary = crate::numerical_audit::read_dictionary(root)?;
    let protocol = json!({"status":"development_only","states":[[-0.0173,0.0107,6.1],[0.0093,0.0201,3.9]],"seed":53101,"regions":2,"budget":54,"observation_spacing_m":0.0005,"candidate_spacings_m":[0.001,0.0005],"noise":"original covariance including 24-frame calibration mean uncertainty","dictionary_cache_key":dictionary.cache_key});
    write_new_json(&output.join("protocol.json"), &protocol)?;
    let mut rng = ChaCha8Rng::seed_from_u64(53101);
    let mut rows = Vec::new();
    for (i, truth) in [[-0.0173, 0.0107, 6.1], [0.0093, 0.0201, 3.9]]
        .into_iter()
        .enumerate()
    {
        let signal = Model::new(0.0005).solve(Some(truth))?.signal;
        let noise = measurements::noise(&mut rng);
        let cal = measurements::noise(&mut rng);
        let observation = std::array::from_fn(|k| signal[k] + noise[k] - cal[k] / 24.0_f64.sqrt());
        let start = Instant::now();
        let coarse = Estimator {
            dictionary: &dictionary,
            calibration_frames: 24,
        }
        .estimate(&observation);
        let coarse_seconds = start.elapsed().as_secs_f64();
        let candidate = refine(&dictionary, &observation, 2, 54, |s| {
            Ok(Model::new(0.001).solve(Some(s))?.signal)
        })?;
        let direct = refine(&dictionary, &observation, 2, 54, |s| {
            Ok(Model::new(0.0005).solve(Some(s))?.signal)
        })?;
        let error = |s: [f64; 3]| (s[0] - truth[0]).hypot(s[1] - truth[1]) * 1000.0;
        let row = json!({"truth_for_evaluation_only":truth,"observation":observation,"coarse_error_mm":error(coarse.state),"coarse":coarse,"coarse_seconds":coarse_seconds,"adaptive_error_mm":candidate.candidates.first().map(|c| error(c.state)),"bounded_direct_refined_error_mm":direct.candidates.first().map(|c| error(c.state)),"adaptive":candidate,"bounded_direct_refined":direct});
        write_new_json(&output.join(format!("case-{i}.json")), &row)?;
        rows.push(row);
    }
    write_new_json(
        &output.join("summary.json"),
        &json!({"status":"exploratory_not_validated","protocol_sha256":crate::evaluation::hash(&serde_json::to_vec(&protocol)?),"rows":rows,"numerical_readiness":false,"limitations":["Only two independent development cases; not confirmatory.","Direct refined comparator uses the same bounded measurement-selected cell shortlist, not exhaustive continuous fine-FEM optimization.","Candidate grid search may be less accurate than continuous coarse optimization; preserve any losses.","Neither candidate produces validated physical confidence."]}),
    )?;
    Ok(())
}
