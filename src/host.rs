//! 仅封装 CPR 已有账号与额度事实和插件私有状态
use crate::model::State;
use gateway_plugin_sdk::{
    ErrorCode, PluginFault,
    call::{data::*, host::*},
    client::{HostClient, SessionError},
};
use std::future::Future;

pub fn fault(message: &str) -> PluginFault {
    PluginFault::new(ErrorCode::Fault, message)
}
pub trait Host: Sync {
    fn load(&self) -> impl Future<Output = Result<(State, Option<u64>), PluginFault>> + Send;
    fn save(
        &self,
        state: &State,
        version: Option<u64>,
    ) -> impl Future<Output = Result<u64, PluginFault>> + Send;
    fn quota(&self, account: &str) -> impl Future<Output = Result<QuotaFacts, PluginFault>> + Send;
}
pub struct SdkHost(pub HostClient);
impl SdkHost {
    pub async fn accounts(&self) -> Result<Vec<AccountFacts>, PluginFault> {
        let mut result = vec![];
        let mut cursor = None;
        loop {
            let page = self
                .0
                .account_facts(AccountFactsQuery {
                    provider_id: Some("openai".into()),
                    cursor,
                    limit: 200,
                })
                .await?;
            result.extend(page.accounts);
            cursor = page.next_cursor;
            if cursor.is_none() {
                return Ok(result);
            }
        }
    }
}
impl Host for SdkHost {
    async fn load(&self) -> Result<(State, Option<u64>), PluginFault> {
        let reply = self
            .0
            .call(
                "host.state.get",
                serde_json::to_value(StateGetRequest {
                    namespace: "guard".into(),
                    key: "state".into(),
                })
                .map_err(|_| fault("状态编码失败"))?,
                vec![],
            )
            .await
            .map_err(SessionError::into_plugin_fault)?;
        let result: StateGetResult =
            serde_json::from_value(reply.result).map_err(|_| fault("状态读取失败"))?;
        match result.record {
            Some(record) => Ok((
                serde_json::from_value(record.value).map_err(|_| fault("状态格式不兼容"))?,
                Some(record.version),
            )),
            None => Ok((State::default(), None)),
        }
    }
    async fn save(&self, state: &State, version: Option<u64>) -> Result<u64, PluginFault> {
        let request = StatePutRequest {
            namespace: "guard".into(),
            key: "state".into(),
            value: serde_json::to_value(state).map_err(|_| fault("状态编码失败"))?,
            expected_version: version,
        };
        let reply = self
            .0
            .call(
                "host.state.put",
                serde_json::to_value(request).map_err(|_| fault("状态编码失败"))?,
                vec![],
            )
            .await
            .map_err(SessionError::into_plugin_fault)?;
        let result: StatePutResult =
            serde_json::from_value(reply.result).map_err(|_| fault("状态保存结果未知"))?;
        Ok(result.version)
    }
    async fn quota(&self, account: &str) -> Result<QuotaFacts, PluginFault> {
        self.0
            .quota_facts(QuotaFactsQuery {
                account_id: account.into(),
            })
            .await
    }
}
