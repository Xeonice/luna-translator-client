# 当前客户端规划与附加发布覆盖

## ADDED Requirements

### Requirement: CLP-001

工程系统 MUST 将最新guided目标、当前PRD、已确认旧设计与当前产品源码事实分别记录，并保持新目标的pending_review与产品未执行状态。

#### Scenario: current-prd-and-design-status-remain-explicit

- GIVEN R74最新guided候选、既有确认范围和当前客户端
- WHEN 本轮同步PRD、客户端文档和总计划
- THEN 文档明确当前正文优先、三记录后置默认折叠和必要配置目标，保留业务语义、已确认范围及真实差距，不改FP表、生成对象、产品或新UI，不自动提升设计或运行状态。

### Requirement: CLP-002

工程系统 MUST 按真实源码、正确M/C及Owner记录当前能力、目标差距、复用点和分片依赖。

#### Scenario: client-gaps-have-owned-and-bounded-implementation-plans

- GIVEN 当前阅读/记录/配置组件、端口与三协议实际源码
- WHEN 规划当前卡、独立记录区、注册层、选择器、typed凭据和离线能力
- THEN 现有能力与待实现目标可区分，Rust权威、窄Tauri、独立Core/worker、共享组件及生命周期边界保持，新契约、codec/seal、资源与迁移前置另审，不从本轮文档获得产品目录权限。

### Requirement: CLP-003

工程系统 MUST 在不重写旧97来源、128定义、153组合及111CSV的前提下，为70个当前增量来源登记完整附加执行闭包。

#### Scenario: seventy-source-identities-and-required-variants-are-closed

- GIVEN PA12、OT16、SS12、FL10、GD12及本轮GUI01至GUI08
- WHEN 生成并校验当前覆盖合同
- THEN 每个来源精确绑定权威断言、需求、设计及状态、M/C、Owner、前置、必需层和适用环境；25项服务、13HTTP协议、4离线引擎与四游戏逐变体可追溯，旧来源定义和not_run/历史状态保持。

### Requirement: CLP-004

工程系统 MUST 对缺真实adapter、确认设计或同一最终包证据的当前范围保持准出阻断，并在实际发布路径无条件检查新增闭包。

#### Scenario: current-release-readiness-is-fail-closed

- GIVEN 当前覆盖合同及尚未完成的新产品执行
- WHEN 实际只读readiness评估与release验证运行
- THEN 静态工程合同可单独通过，评估真实返回blocked/not_run并给逐项缺口；设计、fixture、旧包或有限SR6runner不自动获得完整覆盖能力，缺项不得因本轮文档完成而转为产品或发布passed。

### Requirement: CLP-005

工程系统 MUST 对范围缩减、伪造执行及身份漂移执行真实负例并拒绝，运行记录不得以计划勾选替代。

#### Scenario: adversarial-coverage-and-evidence-claims-are-rejected

- GIVEN 合法覆盖合同、真实adapter登记和执行证据格式
- WHEN 删除来源/服务/必需层/游戏、改变协议身份，或以设计冒产品、fixture冒原生、旧源码/包、错误nonce、零断言/skip、秘密泄漏、裁切节点存在、取消当成功及重复事件伪造通过
- THEN 工具明确拒绝相应声明并保持非零失败；真实运行绑定源码、包、Core/资源/环境、命令日志与证据hash，不重写旧计划或原失败。

### Requirement: CLP-006

工程系统 MUST 通过闭合Owner清单、独立审阅和可回滚事务采用本轮精确文档/工具差异。

#### Scenario: exact-r75-documentation-adoption-preserves-authority

- GIVEN R74原始基线及2087源冻结map、最终30项源清单和独立docs18/tool3审阅
- WHEN Root审查generator/packet并准备和采用R75
- THEN prepare仅写候选、无新UI或产品授权，M15仅唯一hash重绑/确定性CSV，R74归档原字节且全部非授权源与原基线metadata保持；先持久回滚前像、验证完整map后出真实回执，失败恢复Main并保留错误，不提升批准、阶段或产品状态。
