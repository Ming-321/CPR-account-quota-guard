# v0.1.2

适配 CPR v3.29.1：SDK 和打包工具固定到官方 v3.29.1，使用 RPC v4、management v2、middleware v5。

- 继续在 attempt 阶段按已选账号拦截
- 保留固定/随机阈值、周期判断、原有设置和私有状态
- 界面和业务代码不变

请通过插件更新或版本切换升级，不要卸载重装。升级后检查原有保护规则和中间件失败策略。

支持 CPR **>=3.29.0、<3.30.0**，提供 Linux x86_64、ARM64 安装包及 SHA-256 校验文件。旧 CPR 请继续使用先前兼容版本。本次本地构建，CI 改为手动触发。

安装步骤与号池边界见 [README](https://github.com/Ming-321/CPR-account-quota-guard#readme)。生产环境需自行升级。

前端 lint、1 项测试、类型检查及生产构建通过。UI 官方发行包经锁文件 SHA-512 校验一致后在临时目录本地提供，仓库依赖和锁文件未更改。

验证：ARM64 Rust 检查和 11 项逻辑测试通过；两个架构完成 release 编译。x86_64 为交叉编译，未执行运行测试。CPR v3.29.1 的宿主安装、管理页面与升级保留状态尚未联调，由使用者安装测试。步骤见 [本地发布与测试清单](https://github.com/Ming-321/CPR-account-quota-guard/blob/main/docs/local-release.md)。
