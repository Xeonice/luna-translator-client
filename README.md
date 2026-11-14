# Luna Translator

当前guided完整源版本已由用户明确审核通过，身份见[源版批准记录](docs/development/design-approval-guided-20261031.json)；其他页面按[全页与状态闭包](docs/development/all-page-design-coverage.md)逐项对齐。设计源批准、独立参照、真实组件和正式包验收分别记录；原29个阅读/配置增量状态继续保留，不能代表整套页面。

Rust/Tauri 游戏阅读客户端，使用独立精简 LunaHook Core 取文。首版目标包含游戏取文、翻译、内置日语分词词典、按需 AI 精读和服务配置。产品已进入研发；最新设计的功能和呈现仍有待对齐项，当前尚未准出。

公开仓库：[Xeonice/luna-translator-client](https://github.com/Xeonice/luna-translator-client)。[GitHub 与冷环境验证](docs/development/github-activation.md)说明固定语言资源、独立审核、Actions 和分支保护。三个大语言文件从受锁 CI 依赖资产恢复；历史报告与生成的 Tauri staging 不进入 Git。

**现行规范入口：[文档首页](docs/README.md)**。从这里查看当前 PRD、设计、架构、实施差距、测试与准出，不再按历史报告或旧包说明实施。

- [首版功能与边界](docs/product/first-product-scope.md)
- [当前设计及确认状态](docs/design/first-product-v1/README.md)
- [客户端现状与对齐任务](docs/development/current-client-alignment-plan.md)
- [开发和构建](docs/development/desktop-client.md)
- [设计一致性与准出机制](docs/development/design-alignment-and-release.md)

开发执行 `npm run verify -- --change <id>`；产品发布另需正式包、独立确认参照、真实执行器及同包四游戏证据。工程检查通过不代表产品通过。仅用户明确要求时提交或推送。

项目及上游许可见 [License](LICENSE) 和固定 Core/第三方资源清单；原始 PRD、批准图、冻结验收定义和历史结果保留。整理前的说明见[历史索引](docs/archive/documentation-r75/index.md)。
