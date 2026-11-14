# 文档同步设计

主文档：`docs/architecture/japanese-study-and-llm.md`第5章；产品侧：`docs/product/model-configuration-prd.md`。目录、职责、用户预设、协议参数、认证、任务身份、错误和逐层验证全部回写原文档，避免并存第二份协议主稿。

ProtocolAdapterRegistry按(protocolId,adapterContractRevision)查找；CompatibilityProfileRegistry按(profileId,profileRevision)校验厂商方言和参数；ProviderPresetRegistry按(presetId,presetRevision)装配数据。未知版本拒绝，不回退latest。用户JSON只能新增已安装协议/profile的数据预设并形成草稿；新协议需Rust模块注册、编译与验收。UI保持现有Port，不直接HTTP/SQL/FFI。

Google OAuth授权身份、epoch与token材料revision分开冻结，刷新轮换使用Vault新材料及CAS，不篡改原冻结语义。数据库第14章是候选字段与关系：adapterBinding为DTO对象，持久化使用独立列；取代旧研究提案中的单JSON列候选。旧DDL/字段字典/Schema合同/配置seal保持原字节，字段批准与生产迁移是后续独立事项。

一致性工具仅检查文档目录与两组生成矩阵；32个正负守卫验证缺项/错配/未知版本/畸形字段和导出源变化拒绝。HTML读取冻结源字节完成检查、渲染及hash，不复制历史游戏证据、不覆盖既有报告。新checks注册到原verify入口，不改变原检查期望或阈值。

本轮不进行协议适配开发、UI设计批准或客户端替换。PA-01～12是新增协议的验收规划；旧MC-18四变体与153组合不代表13/21覆盖。

维护发现：verify.py新增两项文档检查后，M15唯一来源引用须精确重绑整文件hash。仅改implementation-status.json该1个sha256及原render派生engineering-modules.csv该1个evidence_refs；行号/quote/所有行为与状态/历史/包身份/产品源集合不变。该最小机械映射是本轮额外精确路径，不是新的产品状态核对或通过。
