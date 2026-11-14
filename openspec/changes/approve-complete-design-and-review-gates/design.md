# 完整设计组合与实际审阅门禁

## 一、真实用户批准与不变的来源

新增 `docs/development/design-approval-complete-20261031.json` 记录用户明确原话、R79父身份、guided57+history4的61源及固定闭包。组合ID为complete-guided-history-20261031，target仍guided-service-reading-20261030，默认入口沿guided、历史路由指向R79历史候选。旧57源批准记录、61设计源、原pending标签及全部旧PNG/token/source-version/阈值原字节保留。源版有效状态为approved_by_user；新记录不授予独立逐状态参照或产品/原生/服务/游戏/发布通过。

原57源记录与新61组合解析分开；历史工具只要求原57时保留其精确身份，当前目标和当前覆盖使用61组合。运行依赖wallpaper单独绑定而不偷偷扩成第62个设计源，批准记录依赖也单列。篡改目标、范围、用户语句、父基线、源hash或closure必须拒绝，不能自动重绑批准来迁就实现。

## 二、映射与客户端技术设计

历史页会话筛选/分页及恢复状态按已批准的R79真实HTML/API映射，history20状态为17existing、3unmapped、0missing。当前设计原794状态ID及适用真实层保留，reference/execution/product状态不提升，ADAPTERS注册不被自动制造。新批准不等于已具备当前独立参照或生产执行器。

技术文档、实现状态目标与CP-11明确：游戏/手动来源→会话→记录的稳定身份，历史与会话目录独立50条cursor及冻结ID窗口；追加和替换语义、失败保留/旧范围标注、请求代次拒绝迟到成功与错误；自然新句不抢选中句/词卡/草稿/滚动，显式定位当前句、编辑期间禁用导航及失败保草稿/直接取消。M10/Rust业务与存储权威、窄Tauri、独立Core/worker及既有Repository保持；本轮无生产代码、DDL、协议或技术栈改动。

C23/C24/C36只更新目标/缺口/后续工作说明，状态、现有行为、包、历史与运行事实不提升；M15仅按实际已改质量源重绑来源证据SHA，派生CSV/Markdown严格再生，不扩大legacy-debt或刷新旧违规hash。

## 三、无条件独立变更审阅

`change_review`在verify实际diff已知后无条件执行，不能以kind/tooling、metadata缺省、跳过请求ID或宣称无产品变化免审。metadata.review_evidence固定path和author list：`/root`、`/root/history_design_review`、`/root/complete_design_mapping_docs`、`/root/history_design_governance`。

报告为schema_version=1、kind=independent-change-review/v1，绑定change_id、非空且不属于authors的reviewer_id、与metadata完全一致的author_ids、status=approved、independent_non_author_review=true、无blocking_findings。files覆盖实际45路径的path/bytes/SHA且无重复/遗漏/越界，source_tree_sha256绑定最终完整after。报告放在`.build/current-design-approval-20261031/review/final-change-review.json`，不进入源闭包。metadata不存报告SHA；门禁实际读取并算SHA写执行回执，避免源hash与审阅报告循环。

机器检查证明报告、身份和完整范围一致，不能替代非作者真正阅读与语义审查。必须实际测试缺失、作者自审、错误作者集、旧树、改字节、漏/多文件、重复路径、非法路径、伪通过或blocking意见等拒绝路径，保留完整负例与实际命令输出。

## 四、精确R80采用与恢复

父FP1-20261031-R79：2190源树SHA30a39e0e1d02f56282a4d4420c67451084e25e4031368040761d489d17472952，baseline原字节SHAb5754c102f3ba7c916385ed9128e1a6a1dcce139ab1b056d237bc12805a3cfcf，1317pins。预先ownership SHA b3d24f5b40849f13836ab18e9eae8f7df926e22e1f355fe0a440c4db6b6283dd一次内嵌41source，不能从diff生成许可。

分组最终审阅为authority11、mapping/docs16、Root review-gate/catalog7、change7+helper8。各组要求精确文件身份与独立非作者，Root可审治理组，其他组由非作者audit角色审。prepare只在完整候选生成exact-delta、baseline-review、R79原字节archive及R80baseline四authority，构成45路径完整packet；旧1317pin路径保留。baseline只追加complete_design_source_version并绑定固定recordSHA/61closure，全部旧approvals与其他非revision元数据不变。

独立非作者再审packet并生成上述最终gate报告；两份报告绑定相同最终45文件/after与生成器身份。只有Root可adopt，采用前重验全部原group审阅、packet/generator、gate报告、冻结Main及完整候选。先fsync持久前像和恢复manifest，再写Main；成功完整after相等及baseline/OpenSpec/catalog/scope/gate身份有效，异常恢复原文件/删除新增文件并核完整before留证。采用后再以真实新change_review CLI校验45项及完整树，实际命令/退出码/输出进采用回执，失败进入同一回滚；非优化Python显式拒绝，不声称断电原子性。

实际完整verify保留原30有效checks，并新增change-review-execution、change-review-contract、change-review-tests、current-design-authority、current-design-authority-contract。TS/Rust实际编译、五类负例和history真实设计执行不以登记替代；计划not_run、旧失败、产品blocked、phase/active、技术锁、PNG/阈值与正式包身份保持。
