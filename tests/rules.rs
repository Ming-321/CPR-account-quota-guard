//! 验证额度边界、保守周期识别及配置生效规则
use account_quota_guard::{
    cycle::{SETTLE, Sample, WEEK},
    model::{Guard, Rule},
};

const NOW: i64 = 1_800_000_000_000;
fn sample(observed: i64, reset: i64, used: f64) -> Sample {
    Sample {
        observed,
        reset: Some(reset),
        used,
    }
}
fn fixed() -> Guard {
    Guard::new(true, Rule::Fixed { threshold: 15 })
}
fn random() -> Guard {
    Guard::new(true, Rule::Random { min: 10, max: 20 })
}

#[test]
fn threshold_equality_and_recovery() {
    let mut g = fixed();
    let mut draw = |_, _| panic!("fixed must not randomize");
    assert!(!g.rejects());
    g.observe(sample(NOW, NOW + WEEK / 2, 85.0), &mut draw);
    assert!(!g.rejects());
    g.observe(sample(NOW + 1, NOW + WEEK / 2, 85.1), &mut draw);
    assert!(g.rejects());
    g.observe(sample(NOW + 2, NOW + WEEK / 2, 85.0), &mut draw);
    assert!(!g.rejects());
}
#[test]
fn fixed_edit_is_immediate_but_mode_switch_waits() {
    let mut g = fixed();
    g.observe(sample(NOW, NOW + WEEK / 2, 90.0), &mut |_, _| 12);
    assert!(g.rejects());
    g.configure(true, Rule::Fixed { threshold: 5 });
    assert!(!g.rejects());
    g.configure(true, Rule::Random { min: 50, max: 50 });
    assert_eq!(g.threshold, Some(5));
    g.observe(
        sample(NOW + WEEK, NOW + WEEK + WEEK / 2, 40.0),
        &mut |_, _| 50,
    );
    assert_eq!(g.threshold, Some(50));
}
#[test]
fn random_persists_across_restarts_refreshes_and_toggle() {
    let mut draws = 0;
    let mut draw = |_, _| {
        draws += 1;
        17
    };
    let mut g = random();
    let first = sample(NOW, NOW + WEEK / 2, 20.0);
    g.observe(first.clone(), &mut draw);
    g.observe(first, &mut draw);
    g.observe(sample(NOW + 100, NOW + WEEK / 2, 20.0), &mut draw);
    g.configure(false, g.configured.clone());
    let bytes = serde_json::to_vec(&g).unwrap();
    let mut g: Guard = serde_json::from_slice(&bytes).unwrap();
    g.configure(true, g.configured.clone());
    g.observe(sample(NOW + 200, NOW + WEEK / 2, 30.0), &mut draw);
    assert_eq!(g.threshold, Some(17));
    assert_eq!(draws, 1);
}
#[test]
fn range_edits_wait_and_equal_random_result_is_allowed() {
    let mut g = random();
    g.observe(sample(NOW, NOW + WEEK / 2, 90.0), &mut |_, _| 15);
    g.configure(true, Rule::Random { min: 15, max: 15 });
    assert_eq!(g.current, Rule::Random { min: 10, max: 20 });
    g.observe(
        sample(NOW + WEEK, NOW + WEEK + WEEK / 2, 10.0),
        &mut |a, b| {
            assert_eq!(a, b);
            a
        },
    );
    assert_eq!(g.current, g.configured);
    assert_eq!(g.threshold, Some(15));
}
#[test]
fn early_reset_requires_stable_changed_week_facts() {
    let mut g = random();
    let mut draws = 0;
    let mut draw = |_, _| {
        draws += 1;
        15
    };
    g.observe(sample(NOW, NOW + WEEK / 2, 90.0), &mut draw);
    let early = NOW + 1000 + WEEK;
    g.observe(sample(NOW + 601_000, early, 1.0), &mut draw);
    assert!(g.rejects());
    g.observe(sample(NOW + 601_000 + SETTLE, early, 1.0), &mut draw);
    assert!(
        g.rejects(),
        "a new overall timestamp is not a new weekly observation"
    );
    g.observe(sample(NOW + 601_001 + SETTLE, early, 2.0), &mut draw);
    assert!(!g.rejects());
    assert_eq!(draws, 2);
}
#[test]
fn expired_clock_stale_and_drift_do_not_reset() {
    let mut g = random();
    let reset = NOW + WEEK / 2;
    g.observe(sample(NOW, reset, 90.0), &mut |_, _| 15);
    assert_eq!(g.reason(reset + 1), Some("等待 CPR 更新额度"));
    g.observe(sample(NOW - 1, reset + WEEK, 1.0), &mut |_, _| panic!());
    g.observe(sample(NOW + 1, reset + 60_000, 90.0), &mut |_, _| panic!());
    g.observe(sample(NOW + 2, reset + 130_000, 1.0), &mut |_, _| panic!());
    assert!(g.rejects());
    assert_eq!(g.threshold, Some(15));
}
#[test]
fn floating_unknown_and_high_water() {
    let mut g = random();
    g.observe(sample(NOW, NOW + WEEK, 90.0), &mut |_, _| panic!());
    assert_eq!(g.threshold, None);
    assert!(!g.rejects());
    g.observe(
        sample(NOW + 300_000, NOW + WEEK + 300_000, 91.0),
        &mut |_, _| panic!(),
    );
    assert_eq!(g.threshold, None);
    g.observe(
        sample(NOW + 900_000, NOW + WEEK + 300_000, 92.0),
        &mut |_, _| 15,
    );
    assert!(g.rejects());
    let last = g.latest.clone();
    g.observe(
        sample(NOW + 950_000, NOW + WEEK + 300_000, 92.0),
        &mut |_, _| panic!(),
    );
    g.observe(
        sample(NOW + 910_000, NOW + WEEK + 300_000, 1.0),
        &mut |_, _| panic!(),
    );
    assert_eq!(g.latest, last);
}
#[test]
fn invalid_configuration_and_quota() {
    use account_quota_guard::cycle;
    use gateway_plugin_sdk::call::data::{QuotaFacts, QuotaWindowFacts};
    assert!(!Rule::Random { min: 20, max: 10 }.validate());
    assert!(!Rule::Fixed { threshold: 0 }.validate());
    let mut facts = QuotaFacts {
        schema_version: 1,
        account_id: "a".into(),
        observed_at_ms: Some(NOW),
        windows: vec![QuotaWindowFacts {
            key: "codex:604800s".into(),
            window_seconds: Some(604800),
            used_percent: Some(20.0),
            reset_at_ms: Some(NOW + WEEK / 2),
        }],
    };
    assert!(cycle::sample(&facts, "a", NOW).is_some());
    assert!(cycle::sample(&facts, "b", NOW).is_none());
    facts.windows[0].used_percent = Some(f64::NAN);
    assert!(cycle::sample(&facts, "a", NOW).is_none());
    facts.windows[0].used_percent = Some(20.0);
    facts.windows.push(facts.windows[0].clone());
    assert!(cycle::sample(&facts, "a", NOW).is_none());
}
