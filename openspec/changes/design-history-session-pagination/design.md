# 设计与治理边界

## 交互模型

- 以游戏确定历史范围，明确区分具体游戏、所有游戏和独立手动输入；记录归属始终是 record → session → game，不能靠显示标题推定身份。
- 会话列表和记录列表各自保留50条分页窗口、游标/是否还有更早项、请求代次、等待与错误。会话筛选、搜索和记录范围改变时从对应首批重置；加载更早只追加同一冻结窗口，不偷换当前句或详情对象。
- 等待和失败保留可用旧内容。若旧内容属于上一范围，明确标注，且迟到结果、迟到错误、旧“加载更多”不能污染新范围。重试沿用失败的替换/追加语义。
- 选中旧句、词卡与人工修正固定对象身份；分页不切换详情，当前句定位为显式动作。编辑期间禁用范围切换、分页、刷新和定位；保存失败保留草稿，取消恢复原译文。不得静默覆盖草稿或依赖列表下标归属。
- 主窗、会话列表、记录列表和详情可独立滚动，长文、窄窗、200%、浅色、键盘焦点及专用Select需真实HTML操作与截图。评审状态控制放在产品演示之外。

本候选从已批准guided继承只读来源；设计数据和交互都是页面内fixture，不调用生产端口、HTTP、SQL、FFI或原生层。不新建产品状态权威，不复制Rust业务实现。设计的状态契约描述后续实施义务，不宣称既有产品已实现。

## 追溯与执行

HDP-001对应FP-09按游戏/会话与来源；HDP-002对应FP-09完整分页和互不污染，包含初载/追加等待/错误/空态/末尾；HDP-003对应FP-09人工修正与固定旧句、FP-07词卡及请求隔离；HDP-004对应本轮可审阅设计执行与独立批准/生产验收边界；HDP-005对应精确R78来源治理。C23 HistoryList/HistoryDetail、C24 CorrectionEditor、C04专用Select、C08状态、C09页头和C19词卡是后续组件参照；本轮M13仅设计语义，M15负责HTML设计合同与执行。M10历史持久化和Rust权威保留，不修改业务契约或DDL。

合同检查、合同负例与实际Playwright执行分离：`history-design-contract`检查来源、pending身份和计划；`history-design-tests`执行有意义的合同篡改负例；`history-design-execution`运行真实候选HTML、操作状态、验证断言并生成独立截图/回执。必须绑定源hash、实际命令、当前执行和截图，不靠计划勾选或预写结果通过。执行状态写独立work运行目录；新机器计划的全部执行项保持 `not_run`，缺外部条件用独立回执 `blocked`。

## 精确采用事务

父版本FP1-20261031-R77，raw SHA256 `8c461c9fd548ca204956349c9c1a62402003db7186840e03205ba1824358ef43`；2160源树SHA256 `6616520bd5ace71ce46183af5d963f7bf7f0a44a97f2d7d0ca45cb9f087607e1`，1287pins。ownership在编辑前冻结23个精确路径；helper将其一次内嵌并校验原授权hash，不能从diff生成允许路径。

设计4源必须经非作者审阅，docs6、tools4和change7+helper8各自绑定最终字节；Root先读helper与治理内容再prepare。prepare只在完整候选写4authority：exact-delta、baseline-review、R77原字节归档、新R78baseline。所有旧pin路径保留；仅本轮闭合源和新authority重新pin。R77 approvals键值完全保留，其他非revision metadata及生产状态不动。M15仅替换一次verify.py证据SHA及确定性工程模块CSV，其他渲染出口不变。

最终packet包含完整before/after源图、精确manifest、生成器身份和四组审阅引用；另一个非作者审packet并绑定manifest/generator后，只有Root可adopt。adopt重查Main冻结、候选完整after、审阅与生成器，写入前持久保存每个前像及恢复manifest；成功须完整Main等于after且baseline/scope/catalog/OpenSpec有效。异常删除新文件、恢复旧字节，并以完整before核对恢复，失败单独留证。此事务不声称断电原子性；不可依靠更新审批、旧PNG或技术锁绕过门禁。

## 保留的阻断

当前guided目标、批准记录、57源闭包、旧PNG/token/source-version/阈值、全页两项known_missing、原产品测试定义及历史失败均原样保留。新候选源版用户评审、独立逐状态参照、生产组件截图执行器、正式同包原生与真实游戏/服务执行分别验收；HTML演示或工程门禁通过不解除这些阻断。
