use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::process::Command;
use triad::abstention::{
    fit, CalibrationFrame, CalibrationSequence, Decision, FitSpecification, PolicyStatus,
};
use triad::certify::{
    betting_p_value, binomial_cdf, certify, evaluate_candidates, hoeffding_bentkus_p_value,
    kl_bernoulli, kl_lower_bound, plan, select, CountTable, Method, Targets,
};
use triad::certify_study::{clopper_pearson, validate_protocol};

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * (1.0 + b.abs())
}

fn spec(alpha: f64, cmin: f64, delta: f64, thresholds: Vec<f64>) -> FitSpecification {
    FitSpecification {
        method_id: "certify.test".into(),
        maximum_accepted_error_rate: alpha,
        minimum_coverage: cmin,
        delta,
        candidate_thresholds: thresholds,
    }
}

/// Random calibration fixture: uniform scores, error probability rising with score.
fn fixture(rng: &mut ChaCha8Rng, n: usize, frames: usize, base: f64) -> Vec<CalibrationSequence> {
    (0..n)
        .map(|i| CalibrationSequence {
            id: format!("seq-{i}"),
            frames: (0..frames)
                .map(|_| {
                    let score: f64 = rng.gen();
                    CalibrationFrame {
                        score,
                        incorrect: rng.gen::<f64>() < base + 0.3 * score.powi(3),
                    }
                })
                .collect(),
        })
        .collect()
}

#[test]
fn binomial_and_divergence_match_reference_values() {
    // scipy.stats.binom.cdf(2, 10, 0.3) = 0.3827827864
    assert!(close(binomial_cdf(2, 10, 0.3), 0.382_782_786_4, 1e-9));
    assert!(close(binomial_cdf(0, 5, 0.5), 0.03125, 1e-12));
    assert_eq!(binomial_cdf(-1, 5, 0.5), 0.0);
    assert_eq!(binomial_cdf(5, 5, 0.5), 1.0);
    // Large n does not underflow to zero before the tail mass appears.
    let tail = binomial_cdf(1100, 25_000, 0.05);
    assert!(tail > 0.0 && tail < 1e-3, "{tail}");
    assert!(binomial_cdf(1300, 25_000, 0.05) > 0.9);
    assert_eq!(kl_bernoulli(0.3, 0.3), 0.0);
    assert!(close(kl_bernoulli(0.0, 0.05), -(0.95_f64).ln(), 1e-14));
    // kl dominates the Pinsker bound 2(p-q)^2.
    for (p, q) in [(0.01, 0.05), (0.2, 0.5), (0.9, 0.6)] {
        assert!(kl_bernoulli(p, q) >= 2.0 * (p - q) * (p - q));
    }
}

#[test]
fn p_values_and_bounds_behave_at_their_edges() {
    assert_eq!(hoeffding_bentkus_p_value(0.06, 1000, 0.05), 1.0);
    assert_eq!(hoeffding_bentkus_p_value(0.0, 1000, 1.0), 0.0);
    assert_eq!(hoeffding_bentkus_p_value(0.0, 1000, 0.0), 1.0);
    assert!(hoeffding_bentkus_p_value(0.02, 2000, 0.05) < 1e-6);
    let hoeffding_only = (-2.0 * 2000.0 * 0.03_f64.powi(2)).exp();
    assert!(hoeffding_bentkus_p_value(0.02, 2000, 0.05) < hoeffding_only);
    for (mean, n) in [(0.7, 50), (0.3, 1000), (0.95, 20)] {
        let kl = kl_lower_bound(mean, n, 0.01);
        let hoeffding = mean - ((1.0_f64 / 0.01).ln() / (2.0 * n as f64)).sqrt();
        assert!(kl >= hoeffding && kl <= mean);
        assert!(kl_bernoulli(mean, kl) >= (1.0_f64 / 0.01).ln() / n as f64 - 1e-12);
    }
    assert_eq!(kl_lower_bound(0.0, 100, 0.05), 0.0);
    assert_eq!(betting_p_value(&[], 0.05, 0.01, false), 1.0);
    assert_eq!(betting_p_value(&[0.0; 10], 1.0, 0.01, false), 1.0);
    assert!(betting_p_value(&vec![0.0; 400], 0.05, 0.01, false) < 0.01);
    assert!(betting_p_value(&vec![1.0; 400], 0.5, 0.01, true) < 0.01);
}

#[test]
fn clopper_pearson_matches_closed_forms() {
    // Zero events: upper = 1 - tail^(1/n).
    assert!(close(
        clopper_pearson(0, 1000, 0.99, true),
        1.0 - 0.01_f64.powf(1.0 / 1000.0),
        1e-9
    ));
    // All events: lower = tail^(1/n).
    assert!(close(
        clopper_pearson(1000, 1000, 0.99, false),
        0.01_f64.powf(1.0 / 1000.0),
        1e-9
    ));
    assert_eq!(clopper_pearson(0, 1000, 0.99, false), 0.0);
    assert_eq!(clopper_pearson(1000, 1000, 0.99, true), 1.0);
    let (lo, hi) = (
        clopper_pearson(50, 1000, 0.99, false),
        clopper_pearson(50, 1000, 0.99, true),
    );
    assert!(lo < 0.05 && 0.05 < hi);
}

#[test]
fn legacy_recomputation_equals_abstention_fit_exactly() {
    let mut rng = ChaCha8Rng::seed_from_u64(91);
    for trial in 0..40 {
        let n = rng.gen_range(2..400);
        let frames = rng.gen_range(1..6);
        let sequences = fixture(&mut rng, n, frames, 0.01 * (trial % 5) as f64);
        let s = spec(
            rng.gen_range(0.01..0.3),
            rng.gen_range(0.05..0.9),
            rng.gen_range(0.01..0.3),
            vec![0.1, 0.3, 0.5, 0.7, 0.9],
        );
        let legacy = fit(&s, &sequences).unwrap();
        let mut table = CountTable::new(s.candidate_thresholds.clone(), frames);
        for seq in &sequences {
            table.push(seq.frames.iter().map(|f| (f.score, f.incorrect)));
        }
        let targets = Targets {
            maximum_accepted_error_rate: s.maximum_accepted_error_rate,
            minimum_coverage: s.minimum_coverage,
            delta: s.delta,
        };
        let recomputed = evaluate_candidates(&table, targets, Method::LegacyHoeffdingRatio);
        for (a, b) in legacy.candidates.iter().zip(&recomputed) {
            assert_eq!(a.observed_coverage, b.observed_coverage);
            assert_eq!(Some(a.coverage_lower_bound), b.coverage_lower_bound);
            assert_eq!(
                Some(a.conditional_error_upper_bound),
                b.conditional_error_upper_bound
            );
            assert_eq!(a.qualifies, b.qualifies);
        }
        assert_eq!(
            legacy.threshold,
            select(&recomputed).map(|i| recomputed[i].threshold)
        );
    }
}

#[test]
fn hoeffding_bentkus_dominates_legacy_on_random_studies() {
    let mut rng = ChaCha8Rng::seed_from_u64(1729);
    let mut legacy_ready = 0;
    let mut strictly_better = 0;
    for _ in 0..300 {
        let n = rng.gen_range(10..3000);
        let frames = rng.gen_range(1..4);
        let base = rng.gen_range(0.0..0.05);
        let sequences = fixture(&mut rng, n, frames, base);
        let s = spec(
            rng.gen_range(0.02..0.4),
            rng.gen_range(0.05..0.8),
            rng.gen_range(0.001..0.3),
            vec![0.2, 0.4, 0.6, 0.8, 1.0],
        );
        let legacy = certify_legacy(&s, &sequences);
        let hb = certify(&s, &sequences, Method::HoeffdingBentkus).unwrap();
        for (l, h) in legacy.iter().zip(&hb.candidates) {
            assert!(!l.qualifies || h.qualifies, "dominance violated");
            assert!(h.coverage_lower_bound >= l.coverage_lower_bound);
        }
        let l_cov = select(&legacy).map(|i| legacy[i].observed_coverage);
        let h_cov = hb.threshold.map(|_| {
            hb.candidates
                .iter()
                .find(|c| Some(c.threshold) == hb.threshold)
                .unwrap()
                .observed_coverage
        });
        assert!(h_cov >= l_cov);
        legacy_ready += usize::from(l_cov.is_some());
        strictly_better += usize::from(h_cov > l_cov);
    }
    assert!(legacy_ready > 10, "fixture should exercise ready policies");
    assert!(strictly_better > 10, "fixture should show gains");
}

fn certify_legacy(
    s: &FitSpecification,
    sequences: &[CalibrationSequence],
) -> Vec<triad::certify::CandidateCertificate> {
    let mut table = CountTable::new(s.candidate_thresholds.clone(), sequences[0].frames.len());
    for seq in sequences {
        table.push(seq.frames.iter().map(|f| (f.score, f.incorrect)));
    }
    let targets = Targets {
        maximum_accepted_error_rate: s.maximum_accepted_error_rate,
        minimum_coverage: s.minimum_coverage,
        delta: s.delta,
    };
    evaluate_candidates(&table, targets, Method::LegacyHoeffdingRatio)
}

#[test]
fn sharper_certificate_needs_far_fewer_sequences_on_a_clean_fixture() {
    // The abstention test fixture: three frames per sequence, no errors.
    let clean = |n: usize| -> Vec<CalibrationSequence> {
        (0..n)
            .map(|i| CalibrationSequence {
                id: format!("independent-{i}"),
                frames: [0.1, 0.2, 0.4]
                    .into_iter()
                    .map(|score| CalibrationFrame {
                        score,
                        incorrect: false,
                    })
                    .collect(),
            })
            .collect()
    };
    let s = spec(0.12, 0.8, 0.1, vec![0.2, 0.5]);
    let smallest = |ready: &dyn Fn(usize) -> bool| (1..2000).find(|&n| ready(n)).unwrap();
    let legacy_n = smallest(&|n| fit(&s, &clean(n)).unwrap().status == PolicyStatus::Ready);
    let hb_n = smallest(&|n| {
        certify(&s, &clean(n), Method::HoeffdingBentkus)
            .unwrap()
            .status
            == PolicyStatus::Ready
    });
    assert!(
        hb_n * 4 <= legacy_n,
        "legacy {legacy_n}, hoeffding-bentkus {hb_n}"
    );
}

#[test]
fn certificates_fail_closed_and_ignore_input_order() {
    let mut rng = ChaCha8Rng::seed_from_u64(7);
    let sequences = fixture(&mut rng, 3000, 2, 0.0);
    let s = spec(0.08, 0.5, 0.05, vec![0.25, 0.5, 0.75, 1.0]);
    for method in [Method::HoeffdingBentkus, Method::Betting] {
        let cert = certify(&s, &sequences, method).unwrap();
        assert_eq!(cert.status, PolicyStatus::Ready, "{method:?}");
        let t = cert.threshold.unwrap();
        assert_eq!(cert.decide(Some(t)), Decision::Accept);
        assert_eq!(
            cert.decide(Some(t + 1e-9)),
            Decision::WithholdAboveThreshold
        );
        assert_eq!(cert.decide(None), Decision::WithholdInvalidScore);
        assert_eq!(cert.decide(Some(f64::NAN)), Decision::WithholdInvalidScore);
        let mut reversed = sequences.clone();
        reversed.reverse();
        let again = certify(&s, &reversed, method).unwrap();
        assert_eq!(
            serde_json::to_value(&cert).unwrap(),
            serde_json::to_value(&again).unwrap()
        );
        let json = serde_json::to_string(&cert).unwrap();
        let mut damaged: triad::certify::Certificate = serde_json::from_str(&json).unwrap();
        damaged.threshold = Some(t + 0.01);
        assert_eq!(damaged.decide(Some(0.0)), Decision::WithholdUncalibrated);
        let mut downgraded: triad::certify::Certificate = serde_json::from_str(&json).unwrap();
        downgraded.method = Method::NaiveFrameBinomial;
        assert_eq!(downgraded.decide(Some(0.0)), Decision::WithholdUncalibrated);
        let mut refused: triad::certify::Certificate = serde_json::from_str(&json).unwrap();
        refused.status = PolicyStatus::Refused;
        assert_eq!(refused.decide(Some(0.0)), Decision::WithholdUncalibrated);
    }
    assert!(certify(&s, &sequences, Method::NaiveFrameBinomial).is_err());
    assert!(certify(&s, &sequences, Method::LegacyHoeffdingRatio).is_ok());
    let mut bad = s.clone();
    bad.candidate_thresholds = vec![0.5, 0.5];
    assert!(certify(&bad, &sequences, Method::HoeffdingBentkus).is_err());
}

#[test]
fn tests_hold_their_level_at_the_boundary() {
    // Bernoulli(alpha) observations sit exactly on H0; rejection must stay <= level.
    let mut rng = ChaCha8Rng::seed_from_u64(4242);
    let (alpha, level, reps, n) = (0.05, 0.05, 2000, 400);
    let (mut hb, mut bet) = (0, 0);
    for _ in 0..reps {
        let xs: Vec<f64> = (0..n)
            .map(|_| f64::from(u8::from(rng.gen::<f64>() < alpha)))
            .collect();
        let mean = xs.iter().sum::<f64>() / n as f64;
        hb += usize::from(hoeffding_bentkus_p_value(mean, n, alpha) <= level);
        bet += usize::from(betting_p_value(&xs, alpha, level, false) <= level);
    }
    // 99.9% binomial margin above the nominal 100 rejections.
    assert!(hb <= 135 && bet <= 135, "hb {hb}, betting {bet}");
}

#[test]
fn planner_prefers_hoeffding_bentkus_and_reports_infeasibility() {
    let targets = Targets {
        maximum_accepted_error_rate: 0.05,
        minimum_coverage: 0.5,
        delta: 0.05,
    };
    let p = plan(0.6, 0.0262, targets, 20).unwrap();
    let legacy = p.legacy_hoeffding_ratio_sequences.unwrap();
    let hb = p.hoeffding_bentkus_sequences.unwrap();
    assert!(hb * 4 < legacy, "legacy {legacy}, hb {hb}");
    let infeasible = plan(0.6, 0.06, targets, 20).unwrap();
    assert!(infeasible.legacy_hoeffding_ratio_sequences.is_none());
    assert!(infeasible.hoeffding_bentkus_sequences.is_none());
    assert!(plan(1.5, 0.01, targets, 20).is_err());
}

#[test]
fn frozen_protocol_matches_source_and_mutations_are_rejected() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let protocol: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("experiments/certify-v1.json")).unwrap())
            .unwrap();
    validate_protocol(&protocol).unwrap();
    for (pointer, value) in [
        ("/master_seed", serde_json::json!(1)),
        ("/replicates_per_cell", serde_json::json!(10)),
        ("/targets/delta", serde_json::json!(0.1)),
        ("/error_models/graded/a3", serde_json::json!(0.2)),
        (
            "/regimes/dependent_frames/frames_per_sequence",
            serde_json::json!(5),
        ),
    ] {
        let mut mutated = protocol.clone();
        *mutated.pointer_mut(pointer).unwrap() = value;
        assert!(validate_protocol(&mutated).is_err(), "{pointer}");
    }
}

#[test]
fn certification_cli_contracts() {
    let bin = env!("CARGO_BIN_EXE_triad");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let demo = root.join("examples/abstention-calibration-demo.json");
    let out = Command::new(bin)
        .arg("certify-abstention")
        .arg("--input")
        .arg(&demo)
        .output()
        .unwrap();
    assert!(out.status.success());
    let cert: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(cert["status"], "refused");
    assert_eq!(cert["method"], "hoeffding_bentkus");
    let out = Command::new(bin)
        .args([
            "plan-certification",
            "--coverage",
            "0.6",
            "--risk",
            "0.0262",
            "--alpha",
            "0.05",
            "--min-coverage",
            "0.5",
            "--delta",
            "0.05",
            "--candidates",
            "20",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let p: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(p["hoeffding_bentkus_sequences"].as_u64().unwrap() > 0);
    for args in [
        vec!["certify-abstention", "--input"],
        vec![
            "certify-abstention",
            "--method",
            "naive-frame-binomial",
            "--input",
            "x",
        ],
        vec!["certify-abstention", "--output", "x"],
        vec!["plan-certification", "--coverage", "0.6"],
        vec!["certify-study", "--output", "x", "--bogus", "y"],
    ] {
        let out = Command::new(bin).args(&args).output().unwrap();
        assert!(!out.status.success(), "{args:?}");
    }
    let help = Command::new(bin)
        .args(["certify-study", "--help"])
        .output()
        .unwrap();
    assert!(help.status.success() && !help.stdout.is_empty());
}
