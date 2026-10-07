//! 组合选号后的中间件和管理页面，互斥区不跨越下游执行
use account_quota_guard::{
    engine::Engine,
    host::{SdkHost, fault},
    model::Rule,
};
use gateway_plugin_sdk::{
    ErrorCode, PluginFault,
    call::management::*,
    client::{PluginBuilder, PluginSession, RequestCall, SessionConfig, TypedCall, TypedReply},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn registration() -> ManagementRegistration {
    ManagementRegistration {
        routes: vec![
            ManagementRoute {
                method: "GET".into(),
                path: "state".into(),
                request_content_types: vec![],
                response_content_types: vec!["application/json".into()],
            },
            ManagementRoute {
                method: "POST".into(),
                path: "settings".into(),
                request_content_types: vec!["application/json".into()],
                response_content_types: vec!["application/json".into()],
            },
        ],
        resources: ["web/index.html", "web/app.js", "web/app.css"]
            .into_iter()
            .map(|path| ManagementResource {
                path: path.into(),
                public: false,
            })
            .collect(),
        pages: vec![ManagementPage {
            id: "guard".into(),
            title: "账号额度保护".into(),
            description: Some("按周额度剩余设置保护规则".into()),
            entry: "web/index.html".into(),
            icon: None,
        }],
        callbacks: vec![],
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    version: Option<u64>,
    account_id: String,
    enabled: bool,
    rule: Rule,
}

async fn manage(call: TypedCall<ManagementRequest>) -> Result<Value, PluginFault> {
    let host = SdkHost(call.host);
    let mut engine = Engine::load(&host).await?;
    let accounts = host.accounts().await?;
    if call.request.method == "POST" {
        let cmd: Settings =
            serde_json::from_slice(&call.payload).map_err(|_| fault("设置格式不正确"))?;
        if cmd.version != engine.version {
            return Err(fault("状态已变化，请刷新列表后重试"));
        }
        if !accounts.iter().any(|a| a.account_id == cmd.account_id) {
            return Err(fault("账号不存在或不是 OpenAI 账号"));
        }
        engine.configure(cmd.account_id.clone(), cmd.enabled, cmd.rule)?;
        engine.observe(&cmd.account_id, now()).await;
    } else {
        for id in engine.state.accounts.keys().cloned().collect::<Vec<_>>() {
            engine.observe(&id, now()).await;
        }
    }
    engine.save().await?;
    let guards: serde_json::Map<_, _> = engine
        .state
        .accounts
        .iter()
        .map(|(id, guard)| {
            (
                id.clone(),
                json!({"guard": guard, "status": guard.status(), "reason": guard.reason(now())}),
            )
        })
        .collect();
    Ok(json!({"version": engine.version, "accounts": accounts, "guards": guards}))
}

async fn check(host: &SdkHost, account: &str) -> Result<bool, PluginFault> {
    // 不同进程代次可能同时保存，版本冲突后重新读取已提交结果
    for _ in 0..3 {
        let mut engine = Engine::load(host).await?;
        if !engine
            .state
            .accounts
            .get(account)
            .is_some_and(|g| g.enabled)
        {
            return Ok(false);
        }
        engine.observe(account, now()).await;
        let blocked = engine
            .state
            .accounts
            .get(account)
            .is_some_and(|g| g.rejects());
        match engine.save().await {
            Ok(()) => return Ok(blocked),
            Err(e) if e.code == ErrorCode::Conflict => continue,
            Err(e) => return Err(e),
        }
    }
    Err(fault("状态正在更新，请重试"))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let lock = Arc::new(Mutex::new(()));
    let management_lock = lock.clone();
    let plugin = PluginBuilder::from_json(include_bytes!("../plugin.json"))?
        .middleware(move |call: RequestCall| {
            let lock = lock.clone();
            async move {
                if let Some(account) = &call.context.account_id {
                    let blocked = {
                        let _guard = lock.lock().await;
                        check(&SdkHost(call.host.clone()), account).await?
                    };
                    if blocked {
                        return Err(PluginFault::new(
                            ErrorCode::Rejected,
                            "账号周额度低于保护阈值，等待 CPR 更新额度",
                        ));
                    }
                }
                call.next.run(call.request).await
            }
        })?
        .management(registration(), move |call| {
            let lock = management_lock.clone();
            async move {
                let _guard = lock.lock().await;
                let (status, body) = match manage(call).await {
                    Ok(value) => (200, value),
                    Err(e) => (400, json!({"error": e.message})),
                };
                Ok(TypedReply::new(ManagementResponse {
                    status,
                    content_type: "application/json".into(),
                    headers: vec![],
                })
                .with_payload(serde_json::to_vec(&body).map_err(|_| fault("页面响应编码失败"))?))
            }
        })?
        .build()?;
    PluginSession::accept(
        tokio::io::stdin(),
        tokio::io::stdout(),
        SessionConfig::default(),
    )
    .await?
    .run(plugin)
    .await?;
    Ok(())
}
