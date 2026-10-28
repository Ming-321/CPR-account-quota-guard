# v0.1.3

适配 CPR v3.34.0：SDK 和打包工具固定到官方 v3.34.0，使用 RPC v7、management v2、middleware v6。

- 继续在 attempt 阶段按已选账号拦截
- 保留固定/随机阈值、周期判断、原有设置和私有状态
- 界面和业务代码不变

请通过插件更新或版本切换升级，不要卸载重装。升级后检查原有保护规则和中间件失败策略。

支持 CPR **>=3.34.0、<3.35.0**，提供 Linux x86_64、ARM64 安装包及 SHA-256 校验文件。旧 CPR 请继续使用先前兼容版本。本次本地构建，CI 改为手动触发。

安装步骤与号池边界见 [README](https://github.com/Ming-321/CPR-account-quota-guard#readme)。生产环境需自行升级。

前端源码、依赖和锁文件未改，复用上版已通过 lint、1 项测试、类型检查和生产构建的页面产物。

验证：ARM64 Rust 检查和 11 项逻辑测试通过；两个架构完成 release 编译。x86_64 为交叉编译，未执行运行测试。CPR v3.34.0 的宿主安装、管理页面与升级保留状态尚未联调，由使用者安装测试。步骤见 [本地发布与测试清单](https://github.com/Ming-321/CPR-account-quota-guard/blob/main/docs/local-release.md)。
