# R25：实机发现的历史行刷新与视觉采集字段修复

状态：已获本轮实施授权，候选实现及独立审阅中；未通过新包回归。父基线为 FP1-20261020-R24，父包为 candidate-final-04。该包完整门禁61阶段60通过，视觉前提字段缺失使整体失败；真实SR连续取文、暂停恢复、日志分页与两种正常退出仅按各自子项保留证据。

| 项目 | 冻结范围 |
| --- | --- |
| 需求与验收 | FP-09/12、RW-17～23；已有正文行晚到译文与修正可观察，全量分页与固定查看范围保持；FP-04/11 的视觉前提证明字段来自实际渲染 |
| 设计与状态 | 当前整稿 `#session-history`、正文历史/全部提取/Hook日志；原16源、37图、acceptance-v4与材质/像素阈值不变 |
| 模块与组件 | M10/M13/M15、C15/C23/C33/C36；UI只经ApplicationPort读取Rust权威行，SQL/DTO/Core/模型业务边界不变 |
| Owner | UI Owner：SessionRecordPanel及React回归；Data QA：真实SQLite回归；Root：采集字段/门禁加严/技术合同和状态指纹；独立Reviewer：语义、旧断言及精确权威变更 |
| 产品允许路径 | `apps/desktop/src/features/reading/SessionRecordPanel.tsx`；不引入新框架或复制翻译业务 |
| 测试允许路径 | `apps/desktop/tests/visual/verify-reviewed-reading.mjs`、`capture-conditions.mjs`；`crates/app-infra/tests/session_records_batch.rs`；`scripts/quality/reading_components.py`及对应测试，仅增加必需检查，不删除旧32项 |
| 配套路径 | 本change工作项/设计/tasks/verification与R25独审记录；技术设计、状态清单及其程序派生文件；R24逐字节归档与新基线 |
| 依赖 | canonical04已自然退出并释放冻结；真实SQLite、固定Playwright、Rust锁、原批准图、同一新包；原生预览仍需实际录屏权限，HUD物理拖动仍需实际移动证据 |

高水位固定记录集合、原文版本、过滤和排序，不冻结已保存行的译文及修正。刷新须遍历已加载的不透明页游标，串行读取并按同一会话/数据集/快照/行身份原子更新；不得仅替换首50条、插入范围外正文、回退尾游标或丢失旧页。失败保留最后成功内容，重试使用相同范围；旧条件迟到响应不得覆盖新条件。暂停跟随时保持逻辑可见行与像素偏移，已有译文不可因重译等待/失败/取消消失。

视觉采集只将同一次实际 `hudFit.clampedHeight` 暴露为消费者必需的 `hudFitHeight`，不从expected制造观测结果。保留消费者对缺字段/错高度的拒绝，以及原失败回执。

完成条件：新增React回归对旧Panel真实失败、修复后原32项与新增用例全部执行；真实SQLite以同cursor证明集合固定而行内容更新；独立代码/架构/权威差异审阅后建立R25，构建新候选、完整门禁通过。新包实机保持正文页挂载等待真实模型完成，不靠下一句/切tab刷新；再按SR→MO→Never7→サクラノ詩10th同包回归，不测Ever17。未运行和外部受阻项分别保留not_run/blocked，不改原计划CSV或追认旧包结果。
