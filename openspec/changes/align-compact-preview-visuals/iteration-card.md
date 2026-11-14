# 当前视觉修正迭代卡

| 字段 | 内容 |
|---|---|
| 身份 | align-compact-preview-visuals；父R83，before2233/tree234a，完整snapshot |
| 授权 | 用户“进行视觉问题处理，整理后进行修正”；直接实施已批准设计，未授权提交/推送 |
| 需求/设计 | FP02/03/04/05/11/12；当前61源与最终CSS级联，VCP001～004 |
| 模块/组件 | M03/M13/M15；C12/C14/C16/C17/C35/C36；新增真实详情登记为C35复用表面 |
| Owner | compact_preview_implementation：C35/详情/Sheet显式appearance及局部CSS；current_visual_tests：真实测试与独立源截图；Root：阅读/文案、闭包映射、状态与治理；独立非作者review |
| 允许路径 | verification.json逐项列出；无Native/Rust/DB/公开DTO/原设计源/token/批准图 |
| 依赖/隔离 | 沿现有ApplicationPort和frame状态，保持订阅身份/epoch/generation/序号/权限策略；Sheet default/library语义保留；不改变游戏尺寸或重启游戏 |
| 验证 | Playwright同数据/状态/主题/容器/字号；当前独立参照与真实组件分离；正常/暂停/过期/权限/关闭/长文/200%/缩略容器/详情恢复；原100/CP01回归，v2真实存储，统一verify |
| 完成 | 真实UI已修且适用当前状态结果真实；全量未实现mapping/参照/Native不能伪绿；旧失败/37与原794义务保留，新增surface不隐藏 |

展示事实/测试宿主补充（修改前登记）：CaptureSheet与ReadingPanel仅传递已有真实TargetCandidate/Game/Binding事实到C35 local展示prop，不改DTO/查询/控制；测试宿主ReviewedSurfaceProbe扩容器/主题/标题/双消费者fixture。ApplicationPort不存在任意文本复制接口，因此详情沿HistoryScreen既有navigator.clipboard.writeText模式，失败明确且保留可选择原文，不扩FFI/公开命令。

边界修正已在实现前记录：当前原型519～521外宽纵排会遮挡操作，本轮产品图框按标准窄卡内部280px上限居中，>=520内容宽才横排；不覆盖批准原图/源，也不把该旧边界差异计零。标准backdrop声明保留、复制焦点和200%详情坐标问题均是实际运行发现的UI缺陷，需修复并单列真实证据。

### 有限 CP01 可见操作标签同步
C35 当前批准短按钮为“暂停”；保留 CP01 原11案例/12操作几何与命中断言，只同步实际可见名称。追加上述3条路径；不减少检查或放宽几何边界。Owner：current_visual_tests。

## CP-01 操作义务补充

真实 C35 新增“预览详情”入口后，CP-01 首次执行在精确操作集合检查中正确失败，首跑回执保留于 `.build/visual-correction-20261104/qa/cp01-dispatch-final/20261104T015131.659235Z-42276be8e7-product-cp01-reading-tests/execution.json`。原 11 case、12 项操作及所有几何/命中断言保留；原预览暂停入口按当前已批准源可见短标签同步为“暂停”，并精确追加“预览详情”为第 13 项，同等测量几何与实际 hit。独立证据 guard 同步此精确集合并拒绝遗漏新入口，不使用 subset、过滤入口或放宽预期来消除失败。

新增 PreviewDetails Sheet 的8项义务令现行794→802；允许 visual_surface_contracts 同步 test_current_design_alignment.py 中固定数量断言，必须独立保留原794状态集合、验证仅新增8项与双层要求；不得仅删除计数断言。

### 统一门禁拒绝后的补正
首轮实际检查拒绝客户端规划的按日期追加章节，以及生命周期所需3项Rust测试漏登记。先停止该失败执行并保留部分日志/未完成标记，再将C35现状整合原表和现行说明，显式追加product-core-tests、product-infra-tests、product-native-ipc-tests并实际运行。原批准、原source-before、首轮R84/审批均保留；不放宽文档或生命周期守卫，不改产品代码。

### 新详情依赖的生命周期登记
实际plan复核同时拒绝PreviewDetails新增critical路径及其literal依赖未登记。本轮允许lifecycle-impact-contract.json为该M13/C35只读呈现组件增加精确路径，沿用父预览身份/上下文/帧事实/隔离4义务，禁止critical豁免或改历史phase/Native选择。允许test_lifecycle_impact.py增加登记与删登记负例，原全部测试不变。Root负责。完整当前Native/React适配缺失仍阻断，登记本身不是产品通过。
