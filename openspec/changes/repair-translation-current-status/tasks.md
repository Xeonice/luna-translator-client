## 1. V3 编码前
- [x] 保留packet02/两个P1独立反例与旧所有执行记录。
- [x] v3.3事务/按对象状态/局部隔离/legacy定长/真实history/attempt/floor设计获精确独审，仅isolated。
- [x] 落25路径preimage、完整OpenSpec/迭代卡、nativeIPC必要检查。

## 2. V3 实施与验证
- [x] DTO/窄query/repository read transaction；保留raw ReadingUnit/frozen/旧tasks（原41项application_runtime通过）。
- [x] typed状态四态与legacy summary、失败revision与attempt buffer proof（15项真实Runtime/SQLite测试通过，含第二引擎拒绝全组回滚）。
- [x] C17 owner hook/active/迟到/持续revision收敛；真实后页fixture闭环（最终Node受控预检100/100、2182断言；不冒充registered或native验收）。
- [ ] 原有效测试+新负例、原100RBC、exporter/TS/nativeIPC/storage/完整verify实际执行。

## 3. 交付
- [ ] 新before/after/diff/commands/evidence与Main0漂移证明；新packet独审。
- [ ] Root精确authority/critical link采用；本候选不得自动执行。
- [ ] 新最终包/真实游戏/final cohort（全部未执行not_run）。

执行回执保存在候选外 `../receipts/`；旧packet02、失败、取消执行和新版实测记录分别保留。当前18项core、170项native IPC通过；原storage注册入口实际执行后被3处旧carrier身份阻断，Rust storage stage未执行，不能记通过。完整verify与新代码独审尚待执行；Main/authority/正式包未采用。


## 4. V3.4 注册组件证据闭包
- [x] 实际registered拒绝保留；exact27新设计/独审与两新增preimage在编码前就绪。
- [x] 唯一sidecar严格登记、canonical receipt绑定和原guard+新正反用例（原61+新增17=78项guard真实通过，原assert调用完整保留）。
- [ ] 新fresh registered100与完整verify实际回执；旧storage carrier拒绝不能自刷pin。


## 5. V3.5 固定正版本边界
- [x] 原packet05/code-review03/P2实证与partial保留；same27设计独审与helper/test前像编码前就绪。
- [x] 新projection typed拒0、保留raw unit0 latest，原15+新增真实第16回归（16/16实际通过；新增测试首轮字段拼写编译失败保留，修为真实learning_object字段后通过，未删改断言）。
- [ ] 新packet delta代码复审后完整verify；Root另做authority/存储/新正式包。


## 6. R63治理候选与后续采用
- [ ] 独审database.rs非DDL事实、三个当前指针及独立篡改拒绝；历史来源保持。
- [ ] 独审schema2 finalR63和五条精确projection依赖；原42方法AST保持、全部45测试与真实新ID缺check拒绝实测。
- [ ] 逐项核对Main205实际源漂移/112引用及精确quote remap；202历史输入与当前artifact不刷新。
- [ ] 独审ignored builder最小UTC日期/公开契约增量变体、精确config与13类输出路径。
- [ ] Root精确集成治理输入后才生成packet，独立packet审阅再Root采用；禁止旧approved review冒充新治理批准。
- [ ] 新源实际storage adapter/完整verify；新包和fresh finalR63另建，旧R59/R62失败不提升。
