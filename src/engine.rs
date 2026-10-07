//! 每次处理加载权威状态，保存成功后才采用新的判断和随机值
use crate::{
    cycle,
    host::{Host, fault},
    model::{Guard, Rule, State},
};
use gateway_plugin_sdk::PluginFault;

pub struct Engine<'a, H> {
    pub host: &'a H,
    pub state: State,
    pub version: Option<u64>,
    original: State,
}
impl<'a, H: Host> Engine<'a, H> {
    pub async fn load(host: &'a H) -> Result<Self, PluginFault> {
        let (state, version) = host.load().await?;
        Ok(Self {
            host,
            original: state.clone(),
            state,
            version,
        })
    }
    pub async fn observe(&mut self, id: &str, now: i64) {
        if let Ok(facts) = self.host.quota(id).await
            && let Some(sample) = cycle::sample(&facts, id, now)
            && let Some(guard) = self.state.accounts.get_mut(id)
        {
            guard.observe(sample, &mut |min, max| rand::random_range(min..=max));
        }
    }
    pub fn configure(&mut self, id: String, enabled: bool, rule: Rule) -> Result<(), PluginFault> {
        if !rule.validate() {
            return Err(fault("阈值须为 1–100 的整数，且下限不大于上限"));
        }
        match self.state.accounts.get_mut(&id) {
            Some(guard) => guard.configure(enabled, rule),
            None => {
                self.state.accounts.insert(id, Guard::new(enabled, rule));
            }
        }
        Ok(())
    }
    pub async fn save(&mut self) -> Result<(), PluginFault> {
        if self.state != self.original {
            self.version = Some(self.host.save(&self.state, self.version).await?);
            self.original = self.state.clone();
        }
        Ok(())
    }
}
