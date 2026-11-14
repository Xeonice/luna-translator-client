# 首版实施技术冻结记录

本轮用户明确授权按已有架构与设计开始开发并测试；按FP1-20261019-R6的B0规则固定具体版本。该记录不提升整稿、数据库字段或发布评审状态。

- Rust工具链1.89.0，edition2024；Tokio1.53.3。
- React19/TypeScript5.9/Vite8/Tauri2沿用锁；新增架构已选React Aria Components1.22.1。
- Rust DTO生成ts-rs12.0.1（官方声明MSRV1.78，Rust1.89已实际编译）；SQLite rusqlite0.40.2 bundled（SQLite/source ID以实际runtime记录），生产profile和dictionary迁移均v1。
- HTTP reqwest0.13.5异步+rustls、eventsource-stream0.2.3、futures-util0.3.34；凭据security-framework3.7.0；UUID1.28.0；worker动态MeCab装载libloading0.8.9。删除Tauri中的blocking reqwest、Lindera/IPADIC与直接hook-client依赖。
- macOS仅sha2增加asm特性（sha2-asm0.6.4）启用ARM硬件SHA；Windowsbroker保持原特性，避免新增x86汇编交叉工具。实测语言资源校验与三次worker请求约0.5秒；不减少校验范围掩盖性能问题。
- 独立Core固定e9242ebac32a09e911f306b7c90714891c217302、ABI1七回调、私有JSONL v2，x86/x64完整同版本三件套。逐文件sha/许可见native/core-runtime.lock.json。
- MeCab0.996源码commit61b90ba6e669dc2d7d533d4a80d206f3b31d52b1；官方UniDic2025.12现代书面语包；中文维基词典2026-10-01 dump/2026-10-18抽取与JMdict2026-10-12英文补充。来源、许可、339651词条/590908释义、逐文件身份见resources/language/manifest.json。原始版权与许可保留。
- 仅首版新profile/schema1与dictionary/index1；此前JSON探针配置不静默迁移；未知/未来SQLite版本、模式及迁移hash不符拒绝，原件保持不变。未声称支持此前不存在的数据库升级路线。
- 每用途默认并发1、默认总待执行32、有界上限128；显式配置并发上限8；独立用途队列。队列超限必须真实失败与诊断，不无界spawn；这些实现容量将在行为测试验证。

具体锁来自本轮真实官方元数据、实际编译/运行与资源完整性校验，不把“最新版”当依据。原技术锁与R6基线原字节归档于本change/previous-technology-lock.json及baselines/FP1-20261019-R6.json。

`libloading` 的产品直接依赖为0.8.9，Tauri现有间接依赖另有0.7.4；两者均由完整Cargo.lock逐字锁定。语义主版本表不把此底层库错误地当作全依赖树只准单一版本的框架约束。未新增替代框架或绕开Cargo边界。

## 集成回归中确认的修正

- 原生窗口URL统一使用`role`；辅助窗口与正式bootstrap共用角色解析并进行原生包实测，不能仅用独立测试入口的解析器。
- IPC按Command穷举授权，设置窗口允许凭据池写入，阅读窗口禁止配置写入；按最小能力补充关闭与拖动窗口权限。精读/重译走异步命令与阻塞数据库执行器。
- 宿主退出最长等待13秒（原监督器7秒正常退出、强制结束自有launcher与输出收尾）；未证实结束时保留租约并报告失败，不退出应用或杀游戏/Bottle。
- 自动重试默认0，显式整数0..3；仅连接中断/超时/5xx采用500/1000/2000毫秒退避。认证失败与429不自动重发，避免忽略供应商限流等待；每次持久化独立attempt，固定原引擎、输入和凭据。未经明确策略不回退或换Key。
- 调度应用前校验并发1..8、队列1..128、超时1000..300000毫秒；不把无效值静默截断为有效配置。
- 游戏参数按既定“模型→全局用途/引擎→游戏覆盖”逐层计算；全局引擎先按稳定ID匹配，否则仅允许同model_id唯一启用项，存在歧义时拒绝继承，不按位置或首项猜测。相关全局层和该游戏术语变化必须使该游戏旧授权失效；显示名变化不取消。
- `product-infra-tests`扩大为实际lib及全部integration targets，包含SQLite、协议、运行时和重试；新增`product-native-ipc-tests`执行原生命令权限和执行器测试。门禁自测、正式存储22项与正式包游戏验收仍各自独立。
