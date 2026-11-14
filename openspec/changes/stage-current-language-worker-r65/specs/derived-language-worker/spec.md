# Derived language worker staging

## ADDED Requirements

### Requirement: LWS-001

系统 MUST 保持派生资源身份、真实验证与产品验收边界。

#### Scenario: stage-exact-current-derived-worker-from-real-build-preimage

- GIVEN Root恢复的真实R64 Main1950前像
- WHEN 在独立候选同步当前构建派生资源
- THEN Root保存的实际new2f构建产物与target/release逐字节相同；在真实Main1950前像与自有WI先保存后，只替换候选资源old18b→new2f。不运行prepare/compiler重写Main，不改worker Rust、协议、词典、资源manifest、Core或guard。

### Requirement: LWS-002

系统 MUST 保持派生资源身份、真实验证与产品验收边界。

#### Scenario: real-supervisor-unicode-and-capture-checks-remain-required

- GIVEN Root恢复的真实R64 Main1950前像
- WHEN 在独立候选同步当前构建派生资源
- THEN 使用完整product-infra-tests注册入口实际执行MeCab首次分析、真实hang后15秒kill/wait、清空监督槽及distinct PID重启；不得过滤为单测代替注册回执。强制原样product-ui-capture-v4，其他标准身份/边界/编译与真实component检查保持。可选v1 JSONL probe单独标非registered，不替代监督器或产品通过。

### Requirement: LWS-003

系统 MUST 保持派生资源身份、真实验证与产品验收边界。

#### Scenario: source-authority-and-final-same-package-regression-stay-separate

- GIVEN Root恢复的真实R64 Main1950前像
- WHEN 在独立候选同步当前构建派生资源
- THEN catalog仅资源一键closure更新，不升绿；资源新增pin只能在source+gov独审及Root实际集成后生成authority候选并再次独审采用。当前final63七WI/原freeze不改也不新增pin，R65采用后另行独审rebase。schema2 final63 ID与全部九义务十四links五十步骤及四游戏保持，当前无包/实机均not_run。
