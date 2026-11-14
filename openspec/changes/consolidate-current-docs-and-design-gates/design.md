# 当前文档与设计对齐合同

## 权威次序与稳定入口

产品行为由当前 PRD 及既有权威 ID 约束；最新 guided 是待评审实施目标；Rust 业务权威、窄 Tauri、独立 Core/语言 worker 与既有模块/组件矩阵约束实现；运行与发布只由实际执行证据判断。文档索引提供一个当前入口，角色登记给每份现行说明分配明确职责，避免不同日期旧段落作为并行当前规范。稳定产品 ID、来源文档路径与必要锚点保留，重排不得重解释已确认行为或缩小测试义务。

20 份旧说明按 R75 原字节归档。manifest 绑定 original、snapshot、bytes、sha256 与父基线；历史快照只供追溯，保留原相对链接的原位置语义，不让归档失效链接冒充当前文档。归档说明使用独立 index，不覆盖根 README 的原始快照。

## 对齐门禁与执行层

文档合同校验当前入口、角色、归档完整性及产品义务保留，并以实际正反例拒绝缺入口、重复权威、来源身份漂移或义务缩减。设计对齐合同连接 latest guided 的精确来源、能力、需求与适用验证层，不把“文件存在”“已注册检查”或设计演示当产品实现完成。

设计对齐工程检查、设计批准与真实产品结果分别记录。新只读 readiness 给出缺少的当前能力、真实层、adapter、设计确认或同包证据；缺项保持 blocked/not_run。旧 current-product-test-coverage 的 70 来源、576 组合、25 服务/13 协议/4 引擎、四游戏及原97/128/153和111CSV仍独立强制，不因新登记存在而替换或降级。只在代码登记的 adapter 能力范围内认可执行，JSON 不能自授覆盖或批准。

统一 verify 执行新增五项检查并保留旧检查、实际编译和五类负例。实际 release 路径同时拒绝新设计对齐缺口及既有产品覆盖缺口；工程评估可验证“当前确实阻断”，不把该工程结果写为产品 passed。已批准 canonical source、PNG、token、阈值与候选目录保持原字节，新 guided 仍 pending_review。

## 闭合治理事务

来源清单由 Root 在编辑前写入 ownership.json 并独立固定：root_docs18、architecture_docs5、archive22、tools5、registry1、remap2、change7，共60个唯一精确路径。不得从 candidate diff 推导许可。产品路径、任何未列 metadata、旧机器计划、生成 product spec 与技术锁不进入本轮权限。

最终文档审阅绑定 root_docs+architecture_docs+archive；工具审阅绑定 tools+registry；治理审阅绑定本 change 七件及新 helper。作者与审阅者分开，全部按最终 path、bytes、sha256 闭合，禁止只核对清单名。仅 M15 verify.py 的唯一来源 hash 可重绑，并确定性导出 engineering-modules.csv；其他 catalog 状态与导出保持原字节。

prepare 先核 R75 raw、2102源完整 map、1228 pins 及 Main 未漂移，再验证精确60源候选差异、归档和三类审阅，随后仅在 candidate 生成 R75 原字节归档、新 exact-delta、新 baseline-review 与 R76 baseline。父基线的所有非授权元数据与旧 pins 保留，不清洗历史状态。

独立 packet review 必须绑定 manifest SHA 和 generator SHA。Root adopt 再核冻结 Main、candidate 全文件与 after-map，先持久化全部回滚前像与 recovery-manifest，才按 manifest 写入 Main；写后验证完整树、scope、baseline、spec 与 catalog 并持久化实际 receipt。任何阶段失败均保留原错误并尝试每一项恢复，报告回滚完整性；本轮不得改变 phase、active change、设计批准、产品或发布状态。
