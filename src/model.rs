//! 账号规则与已确认周期状态，页面只消费后端结论
use crate::cycle::{self, Change, Sample};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum Rule {
    Fixed { threshold: u8 },
    Random { min: u8, max: u8 },
}
impl Rule {
    pub fn validate(&self) -> bool {
        match self {
            Self::Fixed { threshold } => (1..=100).contains(threshold),
            Self::Random { min, max } => *min >= 1 && min <= max && *max <= 100,
        }
    }
    fn draw(&self, draw: &mut impl FnMut(u8, u8) -> u8) -> u8 {
        match self {
            Self::Fixed { threshold } => *threshold,
            Self::Random { min, max } => draw(*min, *max),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Guard {
    pub enabled: bool,
    pub configured: Rule,
    pub current: Rule,
    pub threshold: Option<u8>,
    pub cycle: Option<Sample>,
    pub candidate: Option<Sample>,
    pub latest: Option<Sample>,
    pub observed_high_water: i64,
    pub unconfirmed: bool,
    pub blocked: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub accounts: BTreeMap<String, Guard>,
}

impl Guard {
    pub fn new(enabled: bool, rule: Rule) -> Self {
        let threshold = match rule {
            Rule::Fixed { threshold } => Some(threshold),
            _ => None,
        };
        Self {
            enabled,
            configured: rule.clone(),
            current: rule,
            threshold,
            cycle: None,
            candidate: None,
            latest: None,
            observed_high_water: 0,
            unconfirmed: false,
            blocked: false,
        }
    }
    pub fn configure(&mut self, enabled: bool, rule: Rule) {
        self.enabled = enabled;
        // 固定模式的数值编辑立即生效，模式切换或随机区间编辑等待下一周期
        if matches!(self.current, Rule::Fixed { .. }) && matches!(rule, Rule::Fixed { .. }) {
            self.current = rule.clone();
            if let Rule::Fixed { threshold } = rule {
                self.threshold = Some(threshold);
            }
        }
        self.configured = rule;
        self.judge();
    }
    fn judge(&mut self) {
        if let (Some(threshold), Some(sample)) = (self.threshold, &self.latest) {
            self.blocked = 100.0 - sample.used < f64::from(threshold);
        }
    }
    fn accept_cycle(&mut self, sample: &Sample, draw: &mut impl FnMut(u8, u8) -> u8) {
        self.cycle = Some(sample.clone());
        self.candidate = None;
        self.current = self.configured.clone();
        self.threshold = Some(self.current.draw(draw));
    }
    fn retain_or_tighten(&mut self, sample: Sample) {
        self.unconfirmed = true;
        // 周期未知只阻止重抽和放宽，新用量增加仍能收紧保护
        if self
            .latest
            .as_ref()
            .is_none_or(|old| sample.used > old.used)
        {
            self.latest = Some(sample);
            self.judge();
        }
    }
    pub fn observe(&mut self, next: Sample, draw: &mut impl FnMut(u8, u8) -> u8) {
        if next.observed <= self.observed_high_water {
            return;
        }
        self.observed_high_water = next.observed;
        // CPR 合并快照没有逐窗口时间，相同周事实不能用新的总体时间推动周期
        if self.latest.as_ref().is_some_and(|p| p.same_window(&next)) {
            return;
        }
        match self.cycle.clone() {
            None if next.anchored() => self.accept_cycle(&next, draw),
            Some(reference) => match cycle::classify(&reference, &next) {
                Change::Normal => self.accept_cycle(&next, draw),
                Change::Early => {
                    let stable = self.candidate.as_ref().is_some_and(|first| {
                        next.observed - first.observed >= cycle::SETTLE
                            && next.used != first.used
                            && next
                                .reset
                                .zip(first.reset)
                                .is_some_and(|(a, b)| (i128::from(a) - i128::from(b)).abs() <= 1000)
                    });
                    if stable {
                        self.accept_cycle(&next, draw);
                    } else if self
                        .candidate
                        .as_ref()
                        .is_none_or(|p| p.reset != next.reset)
                    {
                        self.candidate = Some(next.clone());
                    }
                    // 新周期尚未确认，不能利用下降用量解除已有保护
                    if !stable {
                        self.retain_or_tighten(next);
                        return;
                    }
                }
                Change::Same => {
                    self.candidate = None;
                    // 保留原始 reset 锚点，微小漂移不能累积成下一周期
                    if let Some(anchor) = &mut self.cycle {
                        anchor.observed = next.observed;
                    }
                }
                Change::Unexplained => {
                    self.retain_or_tighten(next);
                    return;
                }
            },
            None => {}
        }
        self.unconfirmed = false;
        self.latest = Some(next);
        self.judge();
    }
    pub fn rejects(&self) -> bool {
        self.enabled && self.blocked
    }
    pub fn status(&self) -> &'static str {
        if !self.enabled {
            "保护已关闭"
        } else if self.blocked {
            "额度保护暂停"
        } else if self.latest.is_none() || self.threshold.is_none() {
            "等待额度数据"
        } else {
            "保护中"
        }
    }
    pub fn reason(&self, now: i64) -> Option<&'static str> {
        if self.candidate.is_some() || self.unconfirmed {
            Some("等待确认新的周额度周期")
        } else if self.blocked
            && self
                .latest
                .as_ref()
                .and_then(|s| s.reset)
                .is_some_and(|r| now >= r)
        {
            Some("等待 CPR 更新额度")
        } else if self.threshold.is_none() {
            Some("等待可识别的周额度周期")
        } else if self.latest.is_none() {
            Some("尚无有效周额度，暂时放行")
        } else {
            None
        }
    }
}
