## ADDED Requirements

### Requirement: ENG-001 Executable change mapping
门禁 SHALL 拒绝缺少requirement/scenario到实际注册检查的映射，以及PRD之外的产品需求。

#### Scenario: missing-mapping
- **WHEN** change丢失场景映射或引用未知PRD需求
- **THEN** 规范门禁返回非零，不启动依赖该映射的产品交付

### Requirement: ENG-002 Frontend architecture boundary
门禁 SHALL 对TypeScript源码执行AST边界检查，识别直接系统I/O、Tauri绕行、跨feature私有导入及未分类源码目录。

#### Scenario: frontend-cross-layer
- **WHEN** 前端代码导入被禁止的层或在未允许目录新增组件
- **THEN** 代码检查返回非零；合法公开接口用例通过

### Requirement: ENG-003 Rust module dependency boundary
门禁 SHALL 检查Cargo普通、目标、开发、构建及重命名依赖，阻止领域/基础设施/进程边界反转。

#### Scenario: rust-cross-layer
- **WHEN** app-core或broker引入不允许的HTTP/SQL/Tauri依赖
- **THEN** 检查返回非零，重命名和target配置不能绕过

### Requirement: ENG-004 Frozen debt cannot grow
门禁 SHALL 仅容忍逐条登记且文件hash未变的历史PoC问题；新问题、修改后残留问题及发布中的任何历史问题均拒绝。

#### Scenario: legacy-ratchet
- **WHEN** 旧违规文件被修改但违规仍在，或增加相似违规
- **THEN** 返回非零；仅未变的历史精确项可用于工程准备检查

### Requirement: ENG-005 Actual compiler checks
统一门禁 SHALL 执行真实TypeScript和Rust workspace编译检查；工具缺失、命令失败不可标passed。

#### Scenario: real-compilation
- **WHEN** 执行开发门禁
- **THEN** 实际tsc与cargo check成功才给各自编译回执；不提升产品验收

### Requirement: ENG-006 Declared source and directory scope
门禁 SHALL 根据Git base diff或本地源指纹识别所有新增/修改/删除文件并拒绝未声明改动；CI禁止退回自声明快照。

#### Scenario: source-outside-prd
- **WHEN** 代码新增未声明功能文件或改动允许范围外目录
- **THEN** 返回非零，合法精确范围通过

### Requirement: ENG-007 Design and technology identity
门禁 SHALL 拒绝权威设计源漂移、未经更新决策的产品技术manifest变化及超阈值候选画面。

#### Scenario: design-mismatch
- **WHEN** 修改批准材质源或将实际渲染改成不符的颜色
- **THEN** 源指纹或图像差异检查非零，未变对照通过

#### Scenario: unapproved-framework
- **WHEN** 新增或替换Vue/Redux等未锁定框架依赖，或新增未锁manifest
- **THEN** 技术栈检查返回非零

### Requirement: ENG-008 Artifact-bound release evidence
发布门禁 SHALL 要求同一实际包的完整验收记录、真实文件hash和合法证据路径，不接受缺项、blocked或其他包的passed。

#### Scenario: fake-release-evidence
- **WHEN** 填入假passed、错误hash、越界证据或其他产物结果
- **THEN** 发布门禁返回非零

