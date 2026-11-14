# ADR-004：首版工程、状态与设计系统

日期：2026-10-18 · 状态：本轮选定的技术方案，尚未实施。详细规范见[首版研发基线](first-product-engineering-baseline.md)。本记录补齐ADR-003的产品应用层，不改变Core职责或原始PRD。

## 决策

1. 首版集合固定为FP-01～12：游戏库、关联接入/选流、连续原文译文、内置分词词典、按需精读/语境查词、模型配置、历史、手动输入、偏好与诊断。M1原文包只是技术门禁，不能当完整首版。
2. 保留Tauri 2、React 19、TypeScript strict、Vite与Rust；产品业务由Rust应用核心唯一持有，React各窗是可恢复投影，不由主窗代管网络/持久化。采用版本快照与有界Channel，领域用例不依赖Tauri。
3. 新增规划中的app-core/app-infra与本机language-worker边界。已有hook-client/hook-contracts/lunahook-host保留有效基础，按Core ABI1和JSONL v2迁移；不机械改写全部文件，也不提前建立空模块。
4. 采用SQLite/rusqlite bundled及专用数据库执行器，保存业务和配置revision；JSON仅作bootstrap与导入导出。系统安全存储使用macOS security-framework/Keychain和Windows windows-rs/Credential Manager薄适配，配置仅含版本化引用。
5. 语言采用MeCab C ABI＋固定现代书面UniDic与独立本机worker；中文释义采用中文维基词典日语条目离线索引。精确资源、覆盖、许可、hash必须在集成前锁定，英语JMdict不能替代中文门禁。
6. 模型采用Rust异步HTTP，首版包含OpenAI兼容、Anthropic、Azure传统翻译及用户已运行的本机兼容服务。完整保留多引擎、三层配置、草稿/应用、参数三态、上下文、密钥池、取消与缓存身份，不根据临时单模型草稿缩减PRD。
7. 视觉以当前高保真整稿与已确认磨砂案例为准。React Aria仅提供无默认视觉的复杂交互；自有tokens、CSS Modules、同族SVG与共享阅读组件决定外观。不另引整套主题、路由框架或第二套业务状态库。
8. 实现按Owner与端口组织，目标目录、文件职责和组件清单分别由工程组织规范与CSV维护；开发前锁精确依赖/资源并补齐需求-任务-回归映射。

## 取舍

SQLite代替全量JSON历史，支持多实体引用、版本、原子应用与分页。独立语言进程增加一个小型协议与构建产物，换来原生故障隔离和可终止的超时恢复。薄凭据适配增加两份平台实现，但让秘密的存储方式与错误边界可核查。

React Aria减少自写可访问交互，同时通过ui/primitives避免第三方API渗入全部feature。暂不增加Redux/Zustand/Router/Query、SSR或服务端框架，是按当前桌面多窗和Rust权威状态需求做的边界选择；以后变更须说明触发条件与迁移方案。

## 状态与后果

新增依赖、目录与组件仍为计划。本轮不改应用源码、不安装、不编译或运行游戏，不提交推送。前轮未完成代码草稿不构成已采用方案；B0需逐项隔离/评审后决定复用。技术选型确定不保证所有兼容服务或词典质量可用，正式包仍须完成四游戏、真实服务、离线词典、原生窗口及故障回归。
