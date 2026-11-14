# 首版发布的逐组合执行证据

原契约日期：2026-10-19；当前补充：2026-10-31。此处定义发布证据的可执行身份约束，不声明产品通过。原测试计划与 CSV 保持计划、`not_run` 语义；每轮执行输出独立保存在 `.build/`，不回填原计划状态。

## 为什么 97 个来源 ID 不够

97 个 CR/MC/GL 来源用例只覆盖来源身份。当前有效计划包含机制用例，并把同一用例展开为不同环境变体；`test_plan_contract.required_executions()` 从已验证的计划实际展开 **153 个 `case@variant`**。发布必须同时保留来源 `results` 与完整 `execution_results`，不能拿平面 ID 覆盖全部执行组合。

`spec_contract.validate_release()` 先重新验证计划来源、内容身份、层次与变体环境约束，再读取其必需组合。缺组合、重复或未知组合均失败；调用方不能用较短的 `required_case_ids` 参数缩小这份执行集合。

## 外层报告格式

发布证据 `schema_version` 为 **2**。`results` 沿用已有来源用例与文件/hash 校验；新增 `execution_results` 的每项仅引用一份真实执行器回执：

```json
{
  "schema_version": 2,
  "artifact_sha256": "<最终候选包 SHA-256>",
  "results": [
    {
      "case_id": "CR-01-INSTALL",
      "status": "passed",
      "artifact_sha256": "<同包 SHA-256>",
      "evidence": [{"path": ".build/<run>/observed.png", "sha256": "<文件 SHA-256>"}]
    }
  ],
  "execution_results": [
    {
      "execution_id": "CR-01-INSTALL@DISTRIBUTION",
      "adapter_id": "<代码已注册的实际 adapter>",
      "receipt": {"path": ".build/<run>/execution.json", "sha256": "<回执 SHA-256>"}
    }
  ]
}
```

此例只解释字段；省略的其他来源与组合会使真实发布失败。不得复制示例占位值生成通过报告。

## 实际执行器回执

回执沿用 `product_execution.py` 的命令、执行 nonce、日志和当前源码身份，另须提供完整组合断言结果与最终候选包身份。新增字段是后续真实 adapter 的契约；当前注册一个命令不代表已经产出这些字段。

| 字段 | 程序核对 |
|---|---|
| `schema_version` | 整数 1 |
| `adapter_id`、`command` | 与代码注册表精确相同；捕获命令只额外允许现有执行器绑定的 `--output <回执同目录/capture>`，外层不能自行注册命令或改参数 |
| `status`、`errors`、`exit_code` | 分别为 `passed`、空数组、整数 0 |
| `execution_nonce` | 执行器生成的 32 位小写十六进制值，组合记录与原始日志一致 |
| `source_tree_sha256` | 当前源码树重算值，运行后改源码必须重测 |
| `artifact_sha256` | 与本轮正式候选包精确相同，包括源码测试所关联的候选包 |
| `started_at`、`finished_at` | 带时区的实际运行起止时间；每条断言的 `observed_at` 必须位于区间内 |
| `log`、`log_sha256` | 非空、无符号链接、仓库内路径和实际内容 hash |
| `fixture_substitution` | 明确为布尔 `false`；受控输入可用于计划允许的单元/组件测试，但不能替代该组合要求的真实环境 |
| `product_component_tests_executed` | `source_build` 组合明确为 `true` |
| `product_packaged_native_game_tests_executed` | `final_package` 组合明确为 `true` |
| `execution_results` | 实际执行的逐组合断言，不能只写汇总通过 |

每条组合断言必须提供 `execution_id`、`case_id`、`variant_id`、`status`、`evidence_scope`、`environment_ids`、源码/包/nonce 身份、`fixture_substitution`、`observed_at`、`assertion_count`、`skipped_assertions` 与 `evidence`。断言数为正整数，skip 为整数 0；环境 ID 和证据范围与已验证计划精确相同；`evidence` 至少含一份实际非空文件与 hash。

执行器原始日志逐条输出 `LUNA_RELEASE_RESULT <JSON 对象>`。检查器独立读取日志，要求对应记录与回执中的组合断言完全一致；更改汇总断言数、遗漏日志、重复日志 ID、错误 hash、空证据、旧源码或其他包均拒绝。日志 JSON 拒绝重复键及非 JSON 数值。

## 环境与 adapter 边界

- `source_build`：仅允许 `PRODUCT_TEST_CHECKS` / `PRODUCT_CAPTURE_CHECKS` 中的真实执行命令。单位/协议/组件环境仍按计划核对，不把浏览器设计原型作为产品组件执行。
- `final_package`：仅允许 `PRODUCT_RUNTIME_CHECKS` 中的命令。组件、设计截图、守卫 fixture 与普通配置保存不能代替原生窗口、游戏、真实服务或干净环境的执行。
- 当前 `PRODUCT_RUNTIME_CHECKS` 已登记 `product-packaged-stability-tests`，实际命令为 `python3 scripts/quality/runtime_stability.py`。它只证明实际运行时的SR6稳定性范围；没有完整153条逐组合断言契约，也未登记为最新guided附加闭包的能力。注册命令不表示该轮成功执行，既有失败/部分/历史记录保持，完整发布阻断没有解除。

校验 JSON 与 hash 只能证明证据身份及内部一致性，不能独立证明任意文字所声称的系统事件。完整发布还须由统一门禁实际执行代码注册的受信 adapter，将当次运行 nonce 与回执绑定，并通过各功能层、原生与游戏验收；静态校验不得代替执行。测试中的临时 registry 和合成证据只用于证明守卫接受合法结构、拒绝真实变异，不会注册到生产运行表。

## 回归维护

维护位置为 `scripts/quality/test_spec_contract.py` 的 `ContractTests` 与 `ReleaseTests`，沿用统一门禁的 `spec-contract-tests` / `release-evidence-tests`。实际负例包括：97 个来源全部齐全但漏一个变体、未知/重复组合、组件冒充原生、无 runtime、fixture 替代、零断言/skip、环境改写、nonce/源码/包/命令错误、汇总与日志冲突、证据篡改及用非法计划缩小集合。

RW-01～34 的需求来源已进入 OpenSpec 权威来源目录；其六层覆盖另由 [RW 测试覆盖契约](reading-window-test-coverage.md) 验证。153 组合与 RW 分层覆盖互相补充，任何一项通过都不隐含另一项通过。

## 当前guided补充证据：与原153独立强制

原外层schema2及97来源/153执行组合契约不变。当前70来源/576观察组合定义见[current-product-test-coverage.json](current-product-test-coverage.json)，侧录链接只加入原外层对象的 `current_product_coverage` 字段：

```json
{
  "current_product_coverage": {
    "path": ".build/<real-run>/current-product-coverage.json",
    "sha256": "<该文件真实SHA-256>"
  }
}
```

本示例只是字段说明，不能当执行结果。侧录schema1必须带当前 `registry_sha256`、同一 `artifact_sha256` 和完整 `execution_results`；每行含追加的 `execution_id` 与真实执行器 `receipt` path/hash。附加ID采用 `case@layer@variant`，禁止写入原153 `execution_results`，其旧检查器会正确拒绝未知ID。

实际回执字段沿用源码树/同包hash、执行器生成nonce、精确代码注册command、timezone起止、正整数断言与零skip。每条记录还须与计划精确匹配case/layer/variant/environment/dimensions/evidence_scope；证据kind完整且文件非空、hash一致、无符号链接。实际日志逐条输出 `LUNA_CURRENT_PRODUCT_RESULT <JSON>`，与汇总逐项完全一致；重复键、未知/重复组合、错包/旧源/错命令/错层/缺证据均拒绝。

代码能力表同时限定case、layer、execution ID、scope、command和 `current_product_execution_v1` schema；JSON不能注册命令或能力。source_build真实组件/业务/存储/协议不代替final_package服务/worker/原生/四游戏。设计命令只作为追溯引用，不能借其passed满足产品层。未来adapter还必须接入实际执行调度及原始观察检查，不能只添加表项和手写passed回执。

游戏逐状态观察必须有不同字幕hash、稳定state/page身份及每个游戏/客户端配对和callback证据；Never7不少于3页，四款至少各5状态，SR语言分支为non_japanese。200%需实拍无裁剪、全部动作可达；权限拒绝/取消/撤销不能声称live或破坏独立Hook；Vault/导出/日志需观察没有明文秘密。hash/结构核对只证明证据身份与内部一致性，不能独立证明文本声称的事件。

`current_product_coverage.release_coverage_gaps()`通过原外层path/hash读取侧录；统一release入口无条件执行该检查，不能由change元数据删掉。当前生产能力表为空，故静态 `--check` 可passed，`--assess`必须blocked/exit1，不表示执行产品。原153、RW各真实层、实际存储、设计批准、架构及同包要求继续独立强制；任何一组通过都不代另一组。

当前guided完整候选的 `pending_review` 也是 `release_coverage_gaps` 的独立准出阻断。即使单项执行回执完整，也只能解除该项执行缺口；不能由adapter、JSON或历史整稿批准授予当前设计确认。未来独立确认登记须经评审更新相应合同，不能改本计划状态或消除其他缺项来绕过。

当前补充准出仅覆盖既定macOS arm64+CrossOver同一包：576必需观察；原OT-08/OT-16的J北京/金山Windows原机4项另列 `separate_platform_qualifications`，保持not_run、独立环境/ABI/合法资源/Windows工件，不能当Mac包通过或从目录删掉。机器代码限定这4项单列，不允许JSON任意缩减当前scope；未来Windows发布需独立范围与真实准出。

逐游戏的五状态还分别要求 `game_capture` 和 `client_capture` 的真实文件SHA在状态间唯一；复用同图或改名复制同字节不得冒充不同字幕状态。该核对只证明文件内容身份不同，真实adapter仍须验证字幕、页及callback归属，不能由任意不同图片或声明hash伪造游戏推进。

OT-16当前正式包层按4引擎×4游戏显式展开16项，不能由独立worker检查和四款聚合game结果互相顶替。每项冻结engine ID、contract revision、backend、Mac/CrossOver runtime，并要求精确该引擎/游戏/包/nonce的逐状态实际译文、合法资源版本/文件hash/许可证据及独立质量审阅；SR非日语分支仍如实记录。四款至少各五不同游戏/客户端画面，Never7至少三页。原Windows4资格仍在当前scope之外独立not_run。

OT-16中SR既有中文版只按真实同语种原文、直通、非日语降级或实际非日语翻译分别记录；直通/未调用引擎的证据只能通过非日语分支检查，不得记日语译文质量。其他三款日文游戏必须有该引擎真实调用、原译文及独立日语质量结论。每引擎/游戏的资源状态必须ready、已授权且route支持；缺授权/未知/不支持仍blocked，不因直通、显示中文或其他引擎通过而绕过。
