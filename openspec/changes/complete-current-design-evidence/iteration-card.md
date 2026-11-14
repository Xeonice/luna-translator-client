# GCL · 当前设计证据闭合

- 起点：R84/486b13…；用户“进行补充并处理”授权处理上一轮5项门禁未闭合内容。
- 需求：GCL-001～004；FP-01～12及RW-34当前准出层，M13/M15，C04/C12/C16/C17/C18/C19/C20/C22/C26/C27/C29/C30/C31/C34/C35/C36。
- Owner：Root负责目录/审核loader和统合；design_closure_inventory调查映射；current_executor_seams调查执行器；独立review另派，所有协作者不覆盖他人改动。
- 设计：guided-service-reading-20261030＋R79/complete-guided-history-20261031。C35新候选在preview-contract-20261104，仅供评审，不默许生效。
- 允许路径：以verification.json精确列表为准；若调查需要新路径，编辑前补scope并保存原字节。
- 前置：独立候选与参照审核、真实签名包/桌面/四游戏条件分别核实。审批缺失仅阻塞依赖项，独立代码/映射工作继续。
- 验证：真实Playwright源参照/真实组件与严格像素、元数据正反例、原组件/存储、TS/Rust编译、正常包Native实际回归分层记录。
- 完成：完整真实覆盖通过才可宣称准出；原失败留证，未通过不能通过改预期/阈值/legacy债务消除。

## Native实际验证的最小隔离前置
允许bootstrap.rs与platform/runtime_paths.rs增加显式验收profile选择，复用真实Repository/Vault/Hook，不注入DesignPort/假NativeAPI。仅LUNA_ACCEPTANCE_PROFILE=1且32hex执行nonce时选择系统临时根，数据与日志同步隔离且使用新profile UUID，拒绝与canonical用户profile碰撞；普通启动不变。资源仍来自实际签名bundle，目标游戏锁仍放canonical共享根，不能分裂跨实例锁。无public DTO/DB/Vault协议变更。Owner native_acceptance_roots；Root负责精确lifecycle登记与最后source-evidence同步。

参照CRC/尺寸读取复用reading_alignment_preflight.png_pixels，新增仅结构校验选项，仍核完整PNG chunks/CRC、有界解压长度、filter；原像素解码默认行为和全部原比较不变。避免逐状态反复构造不使用的RGBA数组，不信元数据代替原文件。Root负责，scope先补此helper。


用户已明确采用PC01–05五项预览增量。追加精确路径：`docs/development/design-approval-preview-20261104.json`, `docs/design/first-product-v1/README.md`, `docs/product/reading-window-session-prd.md`, `docs/architecture/reading-window-session-technical-design.md`, `scripts/quality/current_preview_authority.py`, `scripts/quality/test_current_preview_authority.py`。原61源及父批准记录保留；新增C35组件补充批准记录绑定冻结5文件，不把它提升为全页或Native批准。Root负责可执行补充权威与负例；UI Owner只负责批准记录及设计入口说明。

首轮真实执行按失败前置中止并保留sourcef5fe及原始回调：实际生命周期计划还要求product-hook-client-tests，现精确补声明；独立DOM回查区分已映射的三个分词开关与确实缺失的三行为；PNG几何使用锁定Playwright enclosingIntRect的坐标包围取整而非绝对高度误差。原图、RGBA零容差和真实缺项不变。
