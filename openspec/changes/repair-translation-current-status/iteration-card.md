# R63 翻译当前状态迭代卡（编码前）

Owner：本候选AP/AI负责Rust投影；UI负责共享C17；QA负责真实SQLite/runtime与Playwright；Root独占Main/authority/包采用。协作者不独占代码库，不覆盖/回退他人。

授权：Root明确完成已确认旧失败/新成功并存缺陷的独立修复候选；不是正式发布、配置变更或R62重写授权。父源1918/tree d23aacb017d33b9f0e4eb7c876533008555b68292ad40457b9ffcaa17149493e；前像在候选外preimage/，产品编辑开始前已落盘。

追溯：FP-04/06/09/10/12 → AI-CFG-03/13、MC-07/20 → docs/design/first-product-v1/index.html#hud-state（当前/失败/恢复，原批准外观保持）→ M05/M10/M13 → C17/C18 → TCS-001～005。现规范未逐字定义替代算法，本delta明确同稳定lane恢复语义；无新产品能力。

允许修改路径以verification.allowed_paths为唯一清单。v2决策使用typed/versioned admission DAG，撤回rowid/clock/ordinal方案；Rust仅在同事务写新task的因果metadata，不改旧事实。新DTO字段为Rust权威只读projection；不修改request事实/DDL/配置/Core/平台。旧task/attempt/error/operation receipt保留，多引擎稳定ID隔离，完整DB排名不从100行推断。

依赖：本机锁定Rust/npm、真实SQLite、已安装Playwright/Chrome；无真实服务/游戏依赖的受控测试可独立执行。DTO合同与代码须独审，Main权威指纹不得自动刷新。

验收：先失败后显式成功本lane退出当前失败，旧事实仍可查；新running/failed/cancelled真实可见；其他引擎失败保留；配置/迟到/100条裁剪/重放不污染；现有断言保留。运行argv/exit/source/log绑定，截图仅独立候选。完整verify缺authority/设计/adapter时保留失败，不以守卫或编译冒充产品通过。正式包和final cohort not_run。

Root于2026-10-28选择legacy保守unordered直到一次新显式请求；设计v2不得推测旧因果。新增domain类型/graph及infra lane读取helper、tasks admission和存储字段精确测试路径已在编码前增列。


## V3.3 编码前替代合同

packet02已changes_required，两P1不可继承旧通过。采纳design-v3.3-proposal设计SHA255dff9b10d920115206a98617631de758cd21dd0be24f92c0f7430ae483b408，独审design-v3.3-review01/review.json SHA c7d05c4105a890e88c74d96549b6cf1530edabbf92a6231ed8fab22e12ed8750，仅准25精确路径隔离实现。正式before-v3-code-receipt与preimage-v3在候选外。Root持Main/authority/包；本次不更新任何旧pin/debt/final cohort。

Owner：AP/AI负责DTO/端口/domain/DB一致读取与failure revision；PL只窄只读Tauri role/query；UI负责共用C17 owner hook、可见性与typed test port；QA真实后续页/元数据局部隔离/attempt交错/100窗/receipt/原assertions/Playwright。允许路径以verification逐项为准，协作者不独占代码库，不回退他人。新增C17 hook是view缓存，不持业务权威；ReadingUnit/frozen数据不挂live状态。

TCS-001～009覆盖原bug、两P1、真实historyPage后续页→readingUnit→projection→重译→C17、fail commit信号、attempt buffer与dirty/floor。完整verify包含product-native-ipc-tests；原100RBC/oracle与所有有效断言保留。新storage adapter编译闭包/pin若拒绝，Root另做精确scope审阅后再执行，不能用full-infra替代adapter。


## V3.4 编码前验证闭包补充
独审design-v3.4-review01/review.json SHA af1261e5333240e9d6ef78294dab1a32a860c775a23bb2d0ef9372c776a8ca05批准exact27仅isolated；design SHA2e7ee9374cc54ba4bbee77b6e678e23ff2d86388158a8650599f2d2cfbf9d7ac。只新增scripts/quality/reading_components.py和test_reading_components.py，M15/QA负责固定sidecar严格身份/freshness与原断言回归；本agent Owner生产adapter，委托QA Owner单一testfile，不覆盖他人。两新增前像已在preimage-v3.4同Main原1918逐hash留存。原25业务/UI源与§1～10合同不减；mjs只补同一canonical receipt.path，原oracle不改。先完成该WI与preimage再改两个新源；全registered100/完整verify/storage仍必须真执行，旧blocked/terminated不作passed。Root独占Main/authority/最终包。


## V3.5 编码前正版本澄清
独立code-review03/review.json SHA2c25d51ba79478ab87b5eff6d7b5212ca1621fef89aa58563b7f9eeeb1e8978f确认原两P1及其余实现闭合，only revision0 P2 changes_required。design-v3.5-review01/review.json SHA812a10e5af06f4913b30932b598318c3bc9bac62f37170dfe63f60532cdb8098批准设计f2dfb844d5a37df182156214abc866e8ca51ee570900573c4b35e8715becbc08，exact27不加路径。Owner本agent仅改已准helper入口与追加真实第16测试；原15/所有assert保留、raw unit0 latest保持。前像preimage-v3.5保存完整801f...map/helper/test/WI。先WI再code；修后新packet delta独审，再慢fullverify；Main/authority/包仍Root。


## R63治理候选 Owner与精确范围（产品源码采用后，待独审）
Owner：Root独占集成/基线采用；AP独立carrier身份复核；M15独立lifecycle/packet/日期与契约记录审阅。新增治理delta及TCS-GOV-001～003逐项映射，不更改原11TCS及其checks。允许额外路径为verification新增的13个精确authority文件、两个v2当前carrier文档、lifecycle合同及test_lifecycle_impact.py；没有目录授权。原1918 source-snapshot不重拍。依赖fresh code-review04批准与root-integration精确Main；完成条件为治理另审、精确Root采用、实际门禁/adapter回执，而非候选生成。
