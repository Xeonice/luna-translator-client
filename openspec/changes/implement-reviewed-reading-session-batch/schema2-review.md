# schema2窄增量实施评审

独立architect审查与实际SQLite负例发现CR-DB-001：合法v1系统时钟回拨导致迁移拒绝。候选作者修复为结束时间可空/非负，保留原值；Root独立重跑旧/新回拨、负值、hash NUL及FK检查后放行具体schema2实施。原failed候选及实跑证据保留。

冻结设计源为docs/architecture/database/v2/002_session_records.design.sql，SHA ec4d1470721b9d81e20a3c5e1bb12aa6f59a2b3c32f6310691f332394781ca2e。新增capture_operations、hook_events、body_record_positions、reading_preference_settings，旧001/旧431字段合同原字节。Root从这份已独立审阅设计生成独立v2结构/字段/边界合同，生产002由另一Owner维护，验证时逐对象比较，不以生产自生成预期自证。

迁移必须先实际验证v1 identity/FK/integrity，生成独立验证backup后单事务002、迁移链和结构核对；失败原v1不变。旧包拒绝v2，回退依独立pre-v2备份，不伪造反向兼容。raw先持久化、操作1:N桥、旧未知开头显式标识、全量分页高水位/导出pin/TTL和活动会话保留按review.md实施。

本评审只批准已授权范围内具体内部字段/关系实施，不提升用户整体DB设计确认、产品实测或发布状态。真实存储22组/431旧字段和新增边界、备份/恢复路线均须实际执行。
