# 聚合修复后的存储源码身份登记

- work_item: IMP-CAPTURE-SOURCE-IDENTITY-20261019；Owner: root；独立审阅: recovery_backend/backend_review。
- 依据：用户已授权实现与收尾；FP-04/09/12，REQ-005/025；不改变已确认的功能、交互、公共契约或数据库字段关系。
- 触发：同源完整 verify 的 storage-acceptance-contract 拒绝过期 database.rs 完整文件 SHA。原失败回执保留。不是解除存储执行覆盖门禁。
- 审阅：新增内容为取文聚合端口、诊断封口和重启恢复时将既有 open 单元封口的 DML。迁移 DDL、schema、版本、迁移账本哈希算法及建库分支保持原值；不能表述为整个 open 函数或所有 SQL 均未改。
- 范围：仅修改 docs/development/storage-acceptance.json 中一处 source_sha256；保存 R12 原文，并按独立审阅登记 R13，基线 files 中只更新该 JSON 的一个指纹。审批状态、其他185项指纹、执行适配器、断言、阈值、批准图、债务不变。
- 追溯：本 change 新增本卡、baseline-review-r13.json 和 R12 原文；补充 proposal/design/tasks 的身份变更说明。运行验证仍使用原有真实编译、字段/边界、五负例和完整 verify，不修改预期。
- 完成：独立审阅结论与输入哈希保留；精确 diff 校验仅上述允许值变化；完整 verify 不再因过期源码身份失败；真正存储执行和视觉失败仍如实拒绝。重新打包，实机运行记录独立绑定最终源码身份。
