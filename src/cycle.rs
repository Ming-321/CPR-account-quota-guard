//! 从被动周窗口事实识别周期，不按时钟推算恢复
use gateway_plugin_sdk::call::data::QuotaFacts;
use serde::{Deserialize, Serialize};

pub const WEEK: i64 = 604_800_000;
pub const TOLERANCE: i64 = 120_000;
pub const SETTLE: i64 = 300_000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub observed: i64,
    pub reset: Option<i64>,
    pub used: f64,
}

pub fn sample(facts: &QuotaFacts, account: &str, now: i64) -> Option<Sample> {
    if facts.schema_version != 1 || facts.account_id != account {
        return None;
    }
    let mut windows = facts.windows.iter().filter(|w| {
        w.key.starts_with("codex:")
            && (w.window_seconds == Some(604800) || w.key.starts_with("codex:604800s"))
    });
    let w = windows.next()?;
    if windows.next().is_some() || w.key != "codex:604800s" || w.window_seconds != Some(604800) {
        return None;
    }
    let used = w.used_percent?;
    let observed = facts.observed_at_ms?;
    if !used.is_finite()
        || !(0.0..=100.0).contains(&used)
        || observed <= 0
        || observed > now + TOLERANCE
    {
        return None;
    }
    Some(Sample {
        observed,
        reset: w.reset_at_ms.filter(|v| *v > 0),
        used,
    })
}

impl Sample {
    pub fn same_window(&self, other: &Self) -> bool {
        self.reset == other.reset && self.used == other.used
    }
    pub fn anchored(&self) -> bool {
        self.reset.is_some_and(|r| {
            let distance = i128::from(r) - i128::from(self.observed);
            distance > 0 && distance < i128::from(WEEK - TOLERANCE)
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Same,
    Normal,
    Early,
    Unexplained,
}

pub fn classify(reference: &Sample, next: &Sample) -> Change {
    let (Some(old), Some(new)) = (reference.reset, next.reset) else {
        return Change::Unexplained;
    };
    let delta = i128::from(new) - i128::from(old);
    if delta.abs() <= i128::from(TOLERANCE) {
        return Change::Same;
    }
    let start = i128::from(new) - i128::from(WEEK);
    if delta > i128::from(TOLERANCE)
        && next.anchored()
        && start >= i128::from(reference.observed.min(old)) - i128::from(TOLERANCE)
        && start <= i128::from(next.observed) + i128::from(TOLERANCE)
    {
        if start >= i128::from(old) - i128::from(TOLERANCE) {
            Change::Normal
        } else {
            Change::Early
        }
    } else {
        Change::Unexplained
    }
}
