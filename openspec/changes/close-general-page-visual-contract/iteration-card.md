# 通用页视觉闭环迭代卡

| 字段 | 内容 |
|---|---|
| Change/父 | close-general-page-visual-contract / implement-approved-client-plan，原FP1-20261105-R90保持 |
| 授权 | 用户明确处理本次剩余任务至完成；原General V3源已确认，无Git提交推送授权 |
| 输入 | 实装与工作树3c485c2c9302df6bb42e5d539c6e8778278384af8b94e78484972ee09cdc7fbb，原baseline05a00ea5不改 |
| 需求/设计 | FP-11、General V3、C1设置A1000×720/窄760×560、14状态与8基础组合 |
| 模块/组件 | M12/M13/M14/M15，C04/C23/C32/C34；Rust业务权威不变 |
| Owner | 第一批五UI/两test角色已完成；第二批Pref两叶、A/reference/adapter与B Native各归独立owner（admission）；Root批准/文档/包，非作者架构/安全/参照/最终代码审核 |
| 允许路径 | verification.json及.build/general-visual-closure-20261108/admission.json精确声明，新叶编辑前补准入 |
| 前置 | V3与三类补充源/Native参照环境均已批准；14态/27capture映射及A独立WK/B同包隔离渲染已实现；独立参照与最终候选分别审核，批准不授实际通过 |
| 验证 | Playwright同数据/环境整页及0RGBA；十设置页search/dirty/focus/窄/200；source/ref/React/Native分层；TS/Rust真实编译、五负例、同包Native |
| 完成 | 本页所有承诺实际证据闭合；未执行not_run/外部不足blocked/差异failed，全R90不随本页提升 |

当前受影响范围由verification精确路径和admission前像约束：SettingsWindow/WindowFrame与General局部呈现，已确认的读取/失败状态，以及C04有限general-settings呈现变体；默认Select规则、Switch/ARIA选择语义、CAS和系统端口保持。独立原图发现的恢复继承重复可点击按真实draftOverrides派生禁用，草稿提示恢复批准源文案；保存、放弃和迟到隔离继续原流程。A使用生产WindowRoot与隔离memory端口；B在签名包的私有有限组合根运行，业务bootstrap不导测试入口。原红图、失败回执与前像完整保留，参照产生者S1与最终候选S2明确分开且有限renderer/source资产须逐字一致。
