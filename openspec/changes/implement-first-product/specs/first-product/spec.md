## ADDED Requirements

### Requirement: FP-01 首次打开与恢复
The product SHALL implement the current FP-01 requirements using the confirmed architecture and current first-product design.
#### Scenario: product-flow-01
- **WHEN** the user operates this first-product flow
- **THEN** real application state, persistence, failures and recovery SHALL follow the authoritative PRD; incomplete prerequisites SHALL remain blocked rather than simulated.

### Requirement: FP-02 游戏入库与执行环境
The product SHALL implement the current FP-02 requirements using the confirmed architecture and current first-product design.
#### Scenario: product-flow-02
- **WHEN** the user operates this first-product flow
- **THEN** real application state, persistence, failures and recovery SHALL follow the authoritative PRD; incomplete prerequisites SHALL remain blocked rather than simulated.

### Requirement: FP-03 窗口或进程接入与正文
The product SHALL implement the current FP-03 requirements using the confirmed architecture and current first-product design.
#### Scenario: product-flow-03
- **WHEN** the user operates this first-product flow
- **THEN** real application state, persistence, failures and recovery SHALL follow the authoritative PRD; incomplete prerequisites SHALL remain blocked rather than simulated.

### Requirement: FP-04 连续原文译文阅读
The product SHALL implement the current FP-04 requirements using the confirmed architecture and current first-product design.
#### Scenario: product-flow-04
- **WHEN** the user operates this first-product flow
- **THEN** real application state, persistence, failures and recovery SHALL follow the authoritative PRD; incomplete prerequisites SHALL remain blocked rather than simulated.
- **WHEN** selected Hook callbacks contain bounded line fragments or growing text snapshots
- **THEN** Rust SHALL immediately publish immutable open revisions with original raw provenance, aggregate only the same session/source/selection, preserve legitimate repeated dialogue, and schedule automatic translation only for an exact stabilized revision under bounded wait and capacity rules.


#### Scenario: acceptance-visual-comparability
- **WHEN** the actual React UI is compared with the current design
- **THEN** every required state SHALL retain a reviewed matching reference, geometry, data and native/client boundary; unknown or mismatched references SHALL fail rather than be silently replaced, masked or accepted.

### Requirement: FP-05 阅读控制与关联并接入
The product SHALL implement the current FP-05 requirements using the confirmed architecture and current first-product design.
#### Scenario: product-flow-05
- **WHEN** the user operates this first-product flow
- **THEN** real application state, persistence, failures and recovery SHALL follow the authoritative PRD; incomplete prerequisites SHALL remain blocked rather than simulated.
- **WHEN** selection, pause, disconnect, abnormal termination, shutdown or restart ends an open capture object
- **THEN** the existing writer transaction or lifecycle authority SHALL preserve and seal its received source without extra model requests, bypassing CAS or allowing old and new sources to merge.

### Requirement: FP-06 翻译与模型配置
The product SHALL implement the current FP-06 requirements using the confirmed architecture and current first-product design.
#### Scenario: product-flow-06
- **WHEN** the user operates this first-product flow
- **THEN** real application state, persistence, failures and recovery SHALL follow the authoritative PRD; incomplete prerequisites SHALL remain blocked rather than simulated.
- **WHEN** a local or remote configuration test persists a failed record and changes the global commit revision
- **THEN** subscribers and the requesting view SHALL refresh authoritative state while preserving the original failure, unsaved edits and the editor's draft-head basis; the next explicit save SHALL use the refreshed global revision without bypassing a real draft-head conflict or automatically applying configuration.

### Requirement: FP-07 内置词典与词法
The product SHALL implement the current FP-07 requirements using the confirmed architecture and current first-product design.
#### Scenario: product-flow-07
- **WHEN** the user operates this first-product flow
- **THEN** real application state, persistence, failures and recovery SHALL follow the authoritative PRD; incomplete prerequisites SHALL remain blocked rather than simulated.

### Requirement: FP-08 按需AI精读
The product SHALL implement the current FP-08 requirements using the confirmed architecture and current first-product design.
#### Scenario: product-flow-08
- **WHEN** the user operates this first-product flow
- **THEN** real application state, persistence, failures and recovery SHALL follow the authoritative PRD; incomplete prerequisites SHALL remain blocked rather than simulated.
- **WHEN** an SSE model response has started but exceeds its configured deadline
- **THEN** the failure SHALL retain its timeout classification, distinct from transport loss, parser failure and a missing terminal marker; old failures SHALL not turn a later completed task into a failed result.

### Requirement: FP-09 阅读历史及人工修正
The product SHALL implement the current FP-09 requirements using the confirmed architecture and current first-product design.
#### Scenario: product-flow-09
- **WHEN** the user operates this first-product flow
- **THEN** real application state, persistence, failures and recovery SHALL follow the authoritative PRD; incomplete prerequisites SHALL remain blocked rather than simulated.
- **WHEN** an aggregated reading unit gains immutable revisions
- **THEN** history SHALL list its latest aggregation-internal revision once, preserve existing explicit processor-revision history semantics, and keep exact older revisions, source spans, frozen learning objects and their results remain retrievable and unchanged.

### Requirement: FP-10 无游戏手动输入
The product SHALL implement the current FP-10 requirements using the confirmed architecture and current first-product design.
#### Scenario: product-flow-10
- **WHEN** the user operates this first-product flow
- **THEN** real application state, persistence, failures and recovery SHALL follow the authoritative PRD; incomplete prerequisites SHALL remain blocked rather than simulated.

### Requirement: FP-11 阅读偏好及资源
The product SHALL implement the current FP-11 requirements using the confirmed architecture and current first-product design.
#### Scenario: product-flow-11
- **WHEN** the user operates this first-product flow
- **THEN** real application state, persistence, failures and recovery SHALL follow the authoritative PRD; incomplete prerequisites SHALL remain blocked rather than simulated.

### Requirement: FP-12 诊断与恢复
The product SHALL implement the current FP-12 requirements using the confirmed architecture and current first-product design.
#### Scenario: product-flow-12
- **WHEN** the user operates this first-product flow
- **THEN** real application state, persistence, failures and recovery SHALL follow the authoritative PRD; incomplete prerequisites SHALL remain blocked rather than simulated.

#### Scenario: acceptance-storage-production
- **WHEN** the registered locked Rust storage target executes
- **THEN** all existing module checks, field boundary probes, typed business/derived/relationship mappings, non-empty dictionary construction and supported migration/recovery/rejection routes SHALL execute with actual assertions, complete same-run receipts and independently verified closed database snapshots.
