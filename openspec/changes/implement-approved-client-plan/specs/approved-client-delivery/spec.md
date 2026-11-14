# 已批准整页实施

## ADDED Requirements

### Requirement: ACD-001
系统 SHALL 实现必须满足FP-04与现行整页合同，保留历史义务和准确状态。

#### Scenario: plan-approval-and-current-state
- GIVEN 已批准R90计划和对应真实输入
- WHEN 实施并执行准确源码的检查
- THEN 行为、职责、状态和证据分别可核，缺失不得提升为产品完成

### Requirement: ACD-002
系统 SHALL 实现必须满足FP-06与现行整页合同，保留历史义务和准确状态。

#### Scenario: typed-catalog-and-exact-registration
- GIVEN 已批准R90计划和对应真实输入
- WHEN 实施并执行准确源码的检查
- THEN 行为、职责、状态和证据分别可核，缺失不得提升为产品完成

### Requirement: ACD-003
系统 SHALL 实现必须满足FP-11与现行整页合同，保留历史义务和准确状态。

#### Scenario: whole-client-reference-is-independent
- GIVEN 已批准R90计划和对应真实输入
- WHEN 实施并执行准确源码的检查
- THEN 行为、职责、状态和证据分别可核，缺失不得提升为产品完成

#### Scenario: live-ui-review-on-production-components
- GIVEN an explicitly labelled development review using current production React components and the approved C1 client dimensions
- WHEN the developer changes a production component or stylesheet
- THEN a fixed loopback-only preview updates through HMR without packaging, with the actual iframe viewport matching the selected client size and no native or external-service actions
- AND browser review, independent design comparison and packaged-native acceptance remain separate; a large browser with a merely fixed-width inner div cannot stand for the actual client viewport

#### Scenario: main-reading-role-typography-and-action-hierarchy
- GIVEN the same reading object and effective Rust preferences, with the approved Main and HUD role hierarchy
- WHEN a sentence, independent translation, streaming state or user font preference changes
- THEN Main renders source/translation at the role-derived sizes without rewriting the original preferences or applying HUD compact mode; streaming and completed translation retain the same size
- AND all seven common actions remain available in order, annotation controls remain in Settings/HUD, and single-result provenance appears once while multiple-result provenance and manual correction stay explicit
- AND same-data screenshots retain honest counts, permission and capture facts; missing rich-text design references remain blocked rather than disabling real SourceText or altering approved images

#### Scenario: manual-layout-and-source-isolation
- GIVEN the approved manual editor/result layout, the existing saved-service selector and genuine typed reading objects
- WHEN a user edits, submits, navigates, clears input or observes independent translation tasks
- THEN the direct editor, ordered actions and permanent result region preserve real commands, draft state and fixed reading identities; a Hook with no game ID is never mistaken for a manual result
- AND saving, model progress and task cancellation remain distinct; source-language selection is retained and an unavailable effective target-language projection is not invented in the UI
- AND missing editable-language, compact-service and rich-result design states remain explicit reference gaps rather than granting all-page or Native acceptance

#### Scenario: targeted-settings-navigation-retains-drafts
- GIVEN a manual translation settings request and a new or existing settings window
- WHEN the typed translation intent crosses the narrow desktop bridge
- THEN Rust retains a monotonic pending request before show and target event delivery; the settings receiver listens before querying and ignores duplicate or older revisions
- AND only Main can send and only Settings can read; the intent cannot name another category or mutate configuration
- AND same-page requests preserve the existing editor, secrets, test state and unsaved drafts without increasing reset keys; genuine departures still use the original discard confirmation


#### Scenario: hud-service-selector-header-inset

- GIVEN the user-requested HUD spacing delta and the existing shared translation-service selector
- WHEN comfortable, compact or narrow HUD content is rendered
- THEN the26px selector has6px top and bottom inset inside a38px header, with distinct drag and window-tool hit targets
- AND reading-only scrolling retains both fixed toolbars; source, client and Native evidence remain separate from whole-product acceptance

#### Scenario: three-page-layout-design-review

- GIVEN the user's history, settings and stopped-session screenshots and the unchanged approved source composition
- WHEN a design candidate is reviewed at the normal C1 and narrow client dimensions
- THEN filters and history content share tracks, settings rows have safe inner spacing, and stopped sessions prioritize saved text and recovery while preserving a currently verified preview
- AND empty current text does not imply empty history; auxiliary actions visibly respond and unavailable translation exposes recovery without fabricated results
- AND the captured design files remain immutable; the separate exact source approval authorizes implementation but the design review itself grants no reference, Native or product acceptance

#### Scenario: approved-layout-client-implementation

- GIVEN the exact user-approved three-page layout supplement and existing Rust-owned projections
- WHEN history, general settings or a stopped current session is shown in the real client
- THEN history filters/content share16px pane lines, settings keep safe inner spacing and accessible save controls, and stopped sessions keep saved-object actions while removing inactive controls and unbound placeholders
- AND a verified preview remains independently subscribed across capture stop, unbind removes that consumer, and saved actions freeze the actual unit/revision
- AND production-component capture, independent source comparison and the same signed package are separately inspected; source-ready and guard self-tests never grant product release

#### Scenario: approved-general-settings-structure-implementation

- GIVEN the user's exact general-settings V3 approval and existing Rust-owned scoped preferences and OS startup ports
- WHEN general settings is rendered or the user edits global/game reading preferences
- THEN the heading and light scope precede lexical, always-saved records and local OS groups, session operations use typed navigation, and the normal default fits1000x720 while narrow and200% remain operable
- AND typed Session navigation is distinct from Capture: the real MainWindow renders the session page for absent/stopped/active sessions and closes any capture modal, while Capture still opens the chooser and History opens history; request/revision/late-replay and failure checks are preserved, not replaced by queued-request assertions
- AND the approved General heading weight700 and scoped font family are compared alongside geometry and size
- AND per-field inheritance, current saved source, draft-only restore, discard, CAS save/failure and stale response isolation retain their actual authority
- AND startup state is OS-confirmed with independent pending/readback and complete on-demand diagnostics, never fabricated by preference save or claimed reinstall recovery
- AND original source bytes remain captured, relocation changes only two dependency URLs with separately executed raw comparison, and approved source, real components and same-signature Native package remain distinct evidence

### Requirement: ACD-004
系统 SHALL 实现必须满足FP-09与现行整页合同，保留历史义务和准确状态。

#### Scenario: history-queries-and-r79-composition
- GIVEN 已批准R90计划和对应真实输入
- WHEN 实施并执行准确源码的检查
- THEN 行为、职责、状态和证据分别可核，缺失不得提升为产品完成

### Requirement: ACD-005
系统 SHALL 新增wire协议必须按官方明确的各自请求、认证、分流、terminal及错误语义执行，不默换兼容接口。

#### Scenario: native-wire-protocols-remain-distinct
- GIVEN 显式Responses、Gemini、Ollama或LM Studio原生协议和有界请求
- WHEN 真实HTTP listener返回JSON、碎片SSE/NDJSON、截断、工具调用或错误
- THEN 使用各自精确路径/字段/terminal，安全错误不泄露原文或凭据，缺terminal拒绝而非成功
### Requirement: ACD-006
系统 SHALL typed秘密合同必须严格按metadata/schema/kind解码，旧raw Key身份不得由内容猜测改变；秘密不进入普通快照、日志或明文Debug。

#### Scenario: typed-secret-contract-preserves-legacy-key-identity
- GIVEN legacy raw Key或metadata明确的新envelope及有限字段
- WHEN 编解码、字段边界或kind错配发生
- THEN 旧Key原字节保留，新对象严格拒绝未知字段/错schema/kind/NUL/过大，Debug脱敏且短期材料不含任意header/token URL/script

### Requirement: ACD-007 Versioned HTTP identity and typed Vault
The client SHALL persist exact protocol/profile/preset revisions in the independently reviewed schema3 HTTP contract and decode credentials only from trusted metadata. Original schema1/2 migrations, rows, seals and applicable obligations SHALL remain preserved.

#### Scenario: versioned-http-identity-and-typed-vault
- **WHEN** a legacy profile migrates or a typed HTTP service is saved and executed
- **THEN** codec identity, original data, exact adapter binding, pool authority, material kind and schema are verified before secrets or network access

### Requirement: ACD-008 Fixed Google service-account assertion
The client SHALL import bounded typed Google service-account JSON and generate only RS256 assertions for the fixed Google token audience and minimal translation scope. Keys SHALL remain secret-only, failed imports SHALL disclose no input, and cryptography SHALL use the exact independently reviewed libraries.

#### Scenario: bounded-service-account-import-and-fixed-assertion
- **WHEN** service-account files or secret material are imported and signed
- **THEN** bounded reads, valid RSA PKCS8 material, exact claims and signature are verified; unknown types, injected token endpoints, invalid time and wrong key kinds are rejected without exposing materials

### Requirement: ACD-009 Explicit service graph codec
The client SHALL represent HTTP and offline targets with distinct, local immutable graph references and explicit version-three encoding. Existing codecs and seals SHALL remain untouched; cross-version source provenance SHALL not authorize execution or imports.

#### Scenario: explicit-service-graph-codec-and-local-targets
- **WHEN** typed service graphs are encoded or explicitly decoded as codec3 candidates
- **THEN** local target relationships, explicit policy intent, stable semantic order and strict shape are preserved; duplicate identities, wrong variants and self-issued internal legacy origins are rejected

### Requirement: ACD-010 Rust-authoritative guided form
The client SHALL project exact registered service fields through the pure Rust condition and default engine. UI SHALL present this result without reimplementing eligibility, credential interpretation or material requirements.

#### Scenario: rust-authoritative-guided-form-projection
- **WHEN** an exact saved catalogue target, purpose and bounded nonsecret form edits are queried
- **THEN** field visibility, requiredness, defaults, inheritance and blockers reflect the typed catalogue; unknown state remains unknown and caller data cannot override secret, observed or unsupported fields

#### Scenario: source-bound-model-discovery-keeps-original-lifetime
- **GIVEN** a genuine saved service source and an explicit model-list action under AI-CFG02
- **WHEN** source, actor, control, credential authority, cancellation or the caller lifetime changes
- **THEN** one application-owned original operation and budget control the actual source checks, bounded wire and physical drain; late or revoked results cannot become a new success
- **AND** listing and explicit choice do not save, activate, rotate credentials or qualify a purpose; old inputs remain readable and failed/empty/expired results preserve manual entry
- **AND** result payload retention is bounded separately from operation identity; an evicted result cannot make the same operation perform I/O again

### Requirement: ACD-011 Reviewed service storage and atomic selection
The client SHALL preserve original migrations, rows, seals and codec identity while implementing the independently frozen schema4 service graph contract. Saved-service activation SHALL modify only the intended applied closure, preserving draft heads, controls, scope and provenance atomically.

#### Scenario: reviewed-service-storage-and-atomic-selection
- **WHEN** a real profile migrates, saves a typed resource or selects saved services
- **THEN** independent field/FK/closure contracts, actual rollback/backup/old-reader checks, stable control identity and exact CAS protect the data; missing runtime proof cannot grant acceptance

#### Scenario: v2-observation-transport-preserves-real-child-lifetime
- **GIVEN** the same offline operation budget, the original Executor capacity owner and a genuine owned child
- **WHEN** v2 observation receives malformed, oversized, out-of-order or incomplete frames, or the caller cancels or disappears
- **THEN** original slot and physical ownership last until actual child wait and stdio completion; inspection capacity remains a separate file-I/O concern
- **AND** observation facts never grant runtime coverage, Installed status or a text Execute authority; production text sending waits for the real private lease integration
- **AND** original v1 assertions and protocol remain unchanged
