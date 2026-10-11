//! Synthetic-fixture tests of the hourly temporal regression benchmark.
use serde_json::json;
use std::collections::BTreeSet;
use triad::metrology_temporal::*;

fn fixture() -> Manifest {
    let first = days_from_civil(2011, 1, 1);
    let last = days_from_civil(2011, 4, 30);
    let holidays = [
        days_from_civil(2011, 1, 17),
        days_from_civil(2011, 2, 21),
        days_from_civil(2011, 4, 15),
    ];
    let mut hours = Vec::new();
    for d in first..=last {
        let date = date_string(d);
        for h in 0..24u32 {
            if elapsed_hour(&date, h).is_err() {
                continue; // spring-forward 02:00 does not exist
            }
            if date == "2011-04-10" && (3..6).contains(&h) {
                continue; // a recording gap
            }
            let holiday = u8::from(holidays.contains(&d));
            let wd = weekday(d);
            hours.push(HourRow {
                date: date.clone(),
                hour: h,
                holiday,
                workingday: u8::from((1..=5).contains(&wd) && holiday == 0),
                weekday: wd,
                cnt: 20 + (h * 7 + (d - first) as u32 * 3) % 50,
            });
        }
    }
    let present: BTreeSet<i64> = hours
        .iter()
        .map(|r| elapsed_hour(&r.date, r.hour).unwrap())
        .collect();
    let lags = vec![1u32, 2, 3, 24, 168];
    let dev_start = days_from_civil(2011, 2, 1);
    let predictions = |shift: f64| -> Vec<Prediction> {
        hours
            .iter()
            .filter(|r| parse_date(&r.date).unwrap() >= dev_start)
            .filter_map(|r| {
                let e = elapsed_hour(&r.date, r.hour).unwrap();
                lags.iter()
                    .all(|l| present.contains(&(e - i64::from(*l))))
                    .then(|| Prediction {
                        date: r.date.clone(),
                        hour: r.hour,
                        prediction: f64::from(r.cnt)
                            * (1.0 + 0.4 * ((e as f64) * 0.37 + shift).sin()),
                        error_scale: 1.0 + (e % 10) as f64,
                        error_probability: ((e * 37 + shift as i64) % 100) as f64 / 100.0,
                    })
            })
            .collect()
    };
    Manifest {
        schema_version: "triad-temporal-regression-1".into(),
        benchmark_id: "fixture".into(),
        provenance: json!({"source_url":"fixture","doi":"none","sha256":"a".repeat(64),"license":"test","citation":"fixture"}),
        protocol_sha256: "b".repeat(64),
        time_zone_rule: "America/New_York (US rule since 2007)".into(),
        partitions: Partitions {
            training: ["2011-01-01".into(), "2011-01-31".into()],
            development: ["2011-02-01".into(), "2011-02-28".into()],
            calibration: ["2011-03-01".into(), "2011-03-31".into()],
            test: ["2011-04-01".into(), "2011-04-30".into()],
        },
        lags: lags.clone(),
        tolerance: Tolerance {
            absolute: 5.0,
            relative: 0.25,
        },
        tolerance_sensitivity: vec![Tolerance {
            absolute: 3.0,
            relative: 0.15,
        }],
        target_coverage: 0.8,
        reliability_bins: 10,
        conformal_alpha: 0.1,
        degradation_margin: 0.05,
        min_accepted_per_day: 5,
        alert_quantile: 0.8,
        min_condition_days: 5,
        completeness_required: 22,
        abstention_costs: vec![0.25, 0.1],
        practical_threshold: 0.01,
        block_lengths: vec![7, 1],
        bootstrap_replicates: 200,
        bootstrap_seed: 7,
        family_alpha: 0.05,
        equivalence_agreement: 0.95,
        models: vec![
            TemporalModel {
                model_id: "a".into(),
                model_sha256: "c".repeat(64),
                predictions: predictions(0.0),
            },
            TemporalModel {
                model_id: "b".into(),
                model_sha256: "d".repeat(64),
                predictions: predictions(1.0),
            },
        ],
        hours,
    }
}

#[test]
fn calendar_and_dst_mapping() {
    assert_eq!(weekday(days_from_civil(2011, 1, 1)), 6); // Saturday
    assert_eq!(elapsed_hour("2011-01-01", 0).unwrap(), 0);
    assert_eq!(
        elapsed_hour("2011-03-13", 3).unwrap() - elapsed_hour("2011-03-13", 1).unwrap(),
        1
    );
    assert!(elapsed_hour("2011-03-13", 2).is_err());
    assert_eq!(
        elapsed_hour("2011-11-06", 2).unwrap() - elapsed_hour("2011-11-06", 1).unwrap(),
        2
    );
    assert!(parse_date("2011-02-29").is_err());
    assert_eq!(date_string(parse_date("2012-02-29").unwrap()), "2012-02-29");
    assert_eq!(ks_statistic(&[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0]), 0.0);
    assert_eq!(ks_statistic(&[1.0, 2.0], &[3.0, 4.0]), 1.0);
}

#[test]
fn replay_and_tamper() {
    let m = fixture();
    let r = evaluate(&m).unwrap();
    validate_report(&r).unwrap();
    let mut t = r.clone();
    t["models"][0]["monitoring"]["primary"]["decision"] = json!("advantage_established");
    assert!(validate_report(&t).is_err());
    let data = &r["data"]["partitions"]["test"];
    assert_eq!(data["missing_target_hours"], 3);
    assert!(data["present_but_lag_missing"].as_u64().unwrap() > 0);
    // Identity comparison of a policy against itself is exactly zero with a degenerate interval.
    for c in r["models"][0]["monitoring"]["secondary_comparisons"]
        .as_array()
        .unwrap()
    {
        if c["identical_decision_days"]
            == r["models"][0]["monitoring"]["degraded_days"]["test_days"]
        {
            assert_eq!(c["delta_mean_cost_a_minus_b"], 0.0);
        }
    }
}

#[test]
fn strict_manifest_rejections() {
    let mut m = fixture();
    m.hours.swap(10, 11);
    assert!(validate(&m).is_err(), "out-of-order hours");
    let mut m = fixture();
    m.hours[5].workingday = 1 - m.hours[5].workingday;
    assert!(validate(&m).is_err(), "inconsistent workingday");
    let mut m = fixture();
    m.models[0].predictions.pop();
    assert!(validate(&m).is_err(), "missing prediction");
    let mut m = fixture();
    m.models[0].predictions[0].error_probability = 1.5;
    assert!(validate(&m).is_err(), "invalid probability");
    let mut m = fixture();
    m.partitions.calibration[0] = "2011-03-02".into();
    assert!(validate(&m).is_err(), "non-contiguous partitions");
    let mut v = serde_json::to_value(fixture()).unwrap();
    v["independent_replication"] = json!(true);
    assert!(
        serde_json::from_value::<Manifest>(v).is_err(),
        "unknown assurance field"
    );
}

#[test]
fn test_period_counts_cannot_move_fitted_quantities() {
    let m = fixture();
    let before = evaluate(&m).unwrap();
    let mut changed = m.clone();
    for h in &mut changed.hours {
        if h.date.as_str() >= "2011-04-01" {
            h.cnt = h.cnt * 3 + 11;
        }
    }
    let after = evaluate(&changed).unwrap();
    for k in 0..2 {
        let (a, b) = (&before["models"][k], &after["models"][k]);
        assert_eq!(a["selective_threshold"], b["selective_threshold"]);
        assert_eq!(a["conformal"]["quantile"], b["conformal"]["quantile"]);
        for key in [
            "r_cal",
            "design_effect",
            "score_scales_development_p80",
            "thresholds",
            "triad_calibration_alert_days_k",
            "cov_support_max_calibration",
        ] {
            assert_eq!(a["monitoring"][key], b["monitoring"][key], "{key}");
        }
    }
}
