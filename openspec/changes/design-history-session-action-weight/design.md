# 次要会话操作的样式层级

## 已授权变更

定位为历史页会话目录的 `#hx-sessions-more`（现有 `.hx-directory` 中的会话加载/重试操作）。默认态去掉常驻可见底色、边框和隆起感，保留轻量文字操作及原来可点击范围。hover、focus-visible和disabled仍可辨，等待时的spinner/文案、失败重试和末尾禁用不改变；不因收轻隐藏入口或削弱键盘操作。

响应式规则必须保持同一主次关系：深浅色、窄窗、长标签和200%布局缩放中，文字操作、状态和筛选仍可达。只通过候选CSS调整，不改页面结构、请求/游标/数据归属、50条双分页、选择/编辑/取消/迟到隔离或产品组件。

`manifest.json`只重绑 `owned_sources["history.css"]` 为真实修改后SHA，其余JSON键值保持R78一致。index.html与history.js和四文件目录闭包不变；待审身份、57源父批准、父manifest及状态清单不因本轮反馈提升。

## 需求与既有执行

HSW-001：默认次要文字操作层级及深浅/窄窗/200%可达。HSW-002：只改样式且保留键盘/hover/disabled/loading/error语义；原HP-01～12定义与执行器继续完整覆盖。HSW-003：精确R79治理及批准/产品边界。

复用 `history-design-contract`、`history-design-tests`、`history-design-execution`；不为修正样式修改旧测试或放宽预期。Root使用Playwright生成独立候选截图并与R78修正前实际图比较，特别核对会话操作、焦点/hover、加载/错误/disabled及布局。统一 `npm run verify -- --change design-history-session-action-weight` 继续执行原全部有效检查、TypeScript/Rust实际编译和PRD/设计/模块/目录/未批准框架五类负例。每轮实际结果单独存work运行目录，不预填通过。

## 精确来源与事务

父版本FP1-20261031-R78；2180源树SHA256 `2fc3ebbdcc116c0fb224df84038d12ec266d8ada6b6fe1e180aacdd71af90338`，父baseline原字节SHA256 `3b51ca08efecd960d57c2d3a38c6fc23d8a71f21782406b39eebe72ce36cac10`，1307pins。完整候选CoW克隆经全树比对；编辑前ownership冻结9source并一次内嵌helper，不从diff授予许可。

源码范围为Root独占CSS/manifest2件与governance Owner新change7件；另仅exact-delta、baseline-review、R78原字节archive及R79baseline四authority。无verify/M15证据重绑、无docs/tools额外审阅范围、无旧change修改。所有原pin路径和旧approvals精确保留；只重新pin本轮闭合源与新增authority，其他非revision元数据不变。

Root先读helper再形成精确8件治理独审；另由非作者审最终design2。prepare只写候选的四authority和完整packet；独立packet审阅绑定manifest、生成器和每个最终文件后，只有Root采用。adopt重核冻结Main、候选完整after、两份原审阅和生成器，Main写入前持久前像与恢复manifest；成功完整after一致，异常恢复原字节/删除新增文件并核对完整before。保持非优化Python拒绝守卫；不声称断电原子性。

当前guided源版批准、待审候选身份、原57源和历史PNG/阈值、全页两项known_missing、所有产品状态/旧计划/phase/active/技术锁均不变。HTML或工程检查仅覆盖其实际层，不授予新源版、独立参照、生产组件或最终包验收。
