## Context

当前目录尚无Git或CI。旧客户端可编译，但仍有直接Tauri调用及旧产品草稿，不能等同新架构已落地。只建立文件规范无法阻止未来代码偏离。

## Goals / Non-Goals

- 建立一个本地/CI共用、失败返回非零的校验入口。
- 对新代码实施边界约束；旧违规逐条绑定文件hash，禁止扩散。
- 规格结构、执行映射、编译、行为、视觉与真实包证据分别判定。
- 本轮不迁移产品、不伪造游戏/模型通过，不将AST或依赖扫描说成完整语义证明。

## Decisions

1. root npm仅固定OpenSpec工具；产品依赖和锁不变。使用现有TypeScript AST和Python标准库tomllib，无新增运行时框架。
2. verification.json一条requirement+scenario对应至少一个注册检查ID；所有delta场景反向核对，禁止漏写、未知检查或只填passed。
3. Git仓库中以显式base diff为源码变更依据；当前无Git以source-snapshot对比。CI禁止退回Git-less快照。
4. 守卫返回原始findings。开发门禁仅容忍与legacy-debt.json完全匹配且整个源文件hash未变化的旧项；发布不接受任何旧项。修改旧文件仍保留违规时拒绝。
5. 编译使用实际tsc和cargo check --workspace --locked --offline；工具缺失或命令失败均失败。编译回执只描述本次输入，不充当产品验收。
6. 发布校验从既有CR/MC/GL矩阵导出首版必需项，绑定一个实际产物和每份证据hash；missing/blocked/其他包/越界路径均拒绝。还须满足批准的设计、资源和零历史债务，不因任务勾选而通过。

## Risks / Limitations

AST及Cargo扫描有明确覆盖范围，不能证明所有状态机语义。OpenSpec结构校验也不读懂代码是否满足句子含义。行为和视觉仍需真实测试；CI文件只有进入正式仓库且配置为必需检查后才有远端合并阻断能力。规则与测试自身也需独立审阅，不防御拥有规则修改权限者蓄意一起篡改检查器。

## Verification

使用真实命令、正反例和临时仓库故障注入，验证新依赖、跨层导入、漏场景、未知检查、源范围逃逸、基线漂移、假证据及不同包均失败。最后核对产品源码字节未变；当前完整产品发布保持未通过。
