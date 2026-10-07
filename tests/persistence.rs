//! 验证额度读取失败、私有状态写入失败与版本冲突
use account_quota_guard::{
    engine::Engine,
    host::{Host, fault},
    model::{Rule, State},
};
use gateway_plugin_sdk::{
    ErrorCode, PluginFault,
    call::data::{QuotaFacts, QuotaWindowFacts},
};
use std::sync::Mutex;

struct FakeHost {
    state: Mutex<(State, Option<u64>)>,
    quota_error: bool,
    save_error: bool,
}
impl Host for FakeHost {
    async fn load(&self) -> Result<(State, Option<u64>), PluginFault> {
        Ok(self.state.lock().unwrap().clone())
    }
    async fn save(&self, value: &State, version: Option<u64>) -> Result<u64, PluginFault> {
        let mut state = self.state.lock().unwrap();
        if self.save_error {
            return Err(fault("failed"));
        }
        if state.1 != version {
            return Err(PluginFault::new(ErrorCode::Conflict, "conflict"));
        }
        let next = version.unwrap_or(0) + 1;
        *state = (value.clone(), Some(next));
        Ok(next)
    }
    async fn quota(&self, id: &str) -> Result<QuotaFacts, PluginFault> {
        if self.quota_error {
            return Err(fault("unavailable"));
        }
        Ok(QuotaFacts {
            schema_version: 1,
            account_id: id.into(),
            observed_at_ms: Some(1_800_000_000_000),
            windows: vec![QuotaWindowFacts {
                key: "codex:604800s".into(),
                window_seconds: Some(604800),
                used_percent: Some(90.0),
                reset_at_ms: Some(1_800_300_000_000),
            }],
        })
    }
}
#[tokio::test]
async fn failure_preserves_last_block_and_first_unknown_allows() {
    let mut host = FakeHost {
        state: Mutex::default(),
        quota_error: true,
        save_error: false,
    };
    let mut engine = Engine::load(&host).await.unwrap();
    engine
        .configure("a".into(), true, Rule::Fixed { threshold: 15 })
        .unwrap();
    engine.observe("a", 1_800_000_000_001).await;
    engine.save().await.unwrap();
    assert!(!engine.state.accounts["a"].rejects());
    host.quota_error = false;
    let mut engine = Engine::load(&host).await.unwrap();
    engine.observe("a", 1_800_000_000_001).await;
    engine.save().await.unwrap();
    assert!(engine.state.accounts["a"].rejects());
    host.quota_error = true;
    let mut engine = Engine::load(&host).await.unwrap();
    engine.observe("a", 1_800_000_900_000).await;
    assert!(engine.state.accounts["a"].rejects());
}
#[tokio::test]
async fn only_one_concurrent_version_commits_and_failed_draw_is_not_published() {
    let mut host = FakeHost {
        state: Mutex::default(),
        quota_error: false,
        save_error: false,
    };
    let mut first = Engine::load(&host).await.unwrap();
    let mut second = Engine::load(&host).await.unwrap();
    for e in [&mut first, &mut second] {
        e.configure("a".into(), true, Rule::Random { min: 10, max: 20 })
            .unwrap();
        e.observe("a", 1_800_000_000_001).await;
    }
    first.save().await.unwrap();
    assert_eq!(second.save().await.unwrap_err().code, ErrorCode::Conflict);
    let loaded = Engine::load(&host).await.unwrap();
    assert_eq!(loaded.state, first.state);
    let saved = first.state.clone();
    drop(first);
    drop(second);
    drop(loaded);
    host.save_error = true;
    let mut failed = Engine::load(&host).await.unwrap();
    failed
        .configure("a".into(), false, Rule::Fixed { threshold: 1 })
        .unwrap();
    assert!(failed.save().await.is_err());
    assert_eq!(Engine::load(&host).await.unwrap().state, saved);
}
