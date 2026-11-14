# 首版发布的逐组合执行证据

当前guided完整源版本已由用户明确审核通过，身份见[源版批准记录](design-approval-guided-20261031.json)；其他页面按[全页与状态闭包](all-page-design-coverage.md)逐项对齐。设计源批准、独立参照、真实组件和正式包验收分别记录；原29个阅读/配置增量状态继续保留，不能代表整套页面。

本合同定义真实执行和证据身份，不声明产品通过。原计划JSON/CSV保留定义及 `not_run`，每次执行另建 `.build/<run_id>/`。当前支持范围为macOS arm64客户端+CrossOver同一最终包；guided与R79完整61源已获用户审核确认；4态独立参照与对应有限真实React能力已登记，其余状态及完整Native缺失，原像素/有效render失败保留，准出仍必须blocked。

## 累计集合与独立侧录

| 合同 | 必需身份 | 判定入口 |
|---|---|---|
| 原首版组合 | 97个CR/MC/GL来源+31机制定义=128定义，展开153个 `case@variant`；原111条CR定义保留 | `spec_contract.validate_release()`独立读取原计划，不能用较短required IDs缩小集合 |
| RW阅读 | RW-01～34的设计/组件/管线/语言/原生/游戏六层 | [RW覆盖合同](reading-window-test-coverage.md)，不由153或设计层抵扣 |
| 存储专项 | schema/字段边界及绑定原用例的实际迁移、事务、故障恢复 | [存储验收](storage-acceptance.md)，需真实生产Rust/SQLite/Repository |
| 当前业务增量 | PA12+OT16+SS12+FL10+GD12+GUI8，共70来源/576观察 `case@layer@variant` | `current_product_coverage.release_coverage_gaps()`读取独立侧录 |
| 当前设计对齐 | guided与R79完整组合及适用状态、61份设计源、精确需求/组件/Owner映射 | `current_design_alignment.release_gaps()`读取独立对齐侧录；候选确认/独立参照/真实React/正式包各自不可缺 |
| Windows资格 | OT-08/OT-16×J北京/金山，共4项 | `separate_platform_qualifications`独立保留not_run及Windows环境/工件，不计入Mac同包576项 |

所有适用集合同时满足才可提交发布评审。旧v4参照仅有历史范围能力，原型、组件、服务、原生或游戏任一层通过均不代表其他层。新观察和新状态不能混入原153的 `execution_results`，也不能由change取消requested checks绕过强制准出。

## 外层报告

发布外层 `schema_version` 为整数2，保留来源 `results` 和原153项 `execution_results`，另以两份path/hash侧录追加当前业务与设计证据：

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
  ],
  "current_product_coverage": {
    "path": ".build/<run>/current-product-coverage.json",
    "sha256": "<侧录 SHA-256>"
  },
  "current_design_alignment": {
    "path": ".build/<run>/current-design-alignment.json",
    "sha256": "<侧录 SHA-256>"
  }
}
```

示例仅解释字段；省略的其他来源/组合必然导致真实发布失败。不得复制占位值或手写passed冒充执行。

## 原153执行器回执

| 字段 | 程序核对 |
|---|---|
| `schema_version` | 整数1 |
| `adapter_id`、`command` | 与代码注册精确相同；捕获只允许受信执行器绑定的额外 `--output <回执同目录/capture>`，JSON不能安装命令或改参数 |
| `status`、`errors`、`exit_code` | `passed`、空数组、整数0 |
| `execution_nonce` | 执行器生成的32位小写十六进制值，逐组合和原始日志一致 |
| `source_tree_sha256` | 当前源码树重算值；运行后改源码须重测 |
| `artifact_sha256` | 同一最终候选包，包括源码层所关联的正式包 |
| `started_at`、`finished_at` | 带时区真实起止；每条 `observed_at`位于区间内 |
| `log`、`log_sha256` | 非空、无符号链接的仓库内实际文件及hash |
| `fixture_substitution` | 布尔false；计划允许的受控输入不能代替该组合要求的真实环境 |
| `product_component_tests_executed` | source_build明确true |
| `product_packaged_native_game_tests_executed` | final_package明确true |
| `execution_results` | 完整逐组合实际断言，不能只写汇总 |

每条断言提供 `execution_id/case_id/variant_id/status/evidence_scope/environment_ids`、源码/包/nonce、fixture标记、观察时间、正整数断言数、整数0的skip和实际非空证据path/hash。环境与scope必须等于验证后计划；遗漏、重复或未知ID均拒绝。

原始日志逐条输出 `LUNA_RELEASE_RESULT <JSON>`，独立读取后与回执逐项一致；错误hash、空证据、旧源码/错包、缺日志、汇总冲突、重复键及非JSON数值均失败。单位/协议/组件source_build只使用已注册的真实源码执行；final_package必须使用实际包运行命令。已注册SR6稳定性runner仅有其具体范围，不获得完整153或当前576业务观察或完整页面状态能力。

## 当前70来源/576观察回执

[current-product-test-coverage.json](current-product-test-coverage.json)保留精确来源断言、需求、Owner/M-C、层、环境与变体。侧录schema1带当前 `registry_sha256`、同一 `artifact_sha256` 和完整 `execution_results`；每行仅以execution ID和path/hash引用真实回执，不注册能力。

回执绑定当前源码、同包、执行nonce、精确代码command、时区起止、正断言/零skip。case/layer/variant/environment/dimensions/scope和证据kind必须精确匹配；文件非空、hash一致、无符号链接。原日志逐条输出 `LUNA_CURRENT_PRODUCT_RESULT <JSON>`并与汇总一致，错层、旧源/错包、伪命令/nonce、缺证据或重复/未知组合均拒绝。代码能力表限定具体case/layer/execution ID/scope/command/schema；未来adapter还必须接实际调度及原始观察检查，不能只加表项。

真实组件/领域/存储/协议不替代final_package的服务、worker、原生、四游戏或独立内容评审。当前新增生产能力表为空；`--check`只验证计划，`--assess`缺能力/证据须blocked/exit1。

## 当前完整页面状态/61源组合设计对齐

[current-design-alignment.json](current-design-alignment.json)固定guided target、guided与R79共61份完整设计源身份和完整页面适用状态的需求、Owner与组件路径。`current_design_alignment.py`区分候选原快照标记、当前源版用户确认、独立参照审阅及批准参照catalogue；不存在以JSON、adapter或历史approval自动提升确认状态的路径。

对齐侧录schema1绑定 `target_id/contract_sha256/artifact_sha256`并引用当前实际捕获receipts。代码登记的adapter必须精确支持当前target、real_react或packaged_native层及所需状态，并具有分离的execute、inspect、独立recompute；原始截图/动作/文本/布局/焦点/快捷键事实由独立代码核对，不能相信JSON自报能力或passed。真实React与正式包分别覆盖，不接受HTML原型互比、旧PNG改名重标、缺状态/重复状态、source/component冒native、旧hash或自生成批准参照。

开发产品UI改动也无条件按实际改动路径检查当前能力；无法确定影响时保守要求完整状态，不由requested checks或旧v4声明解除。release无条件完整检查全部适用状态及两个层。当前仅4态独立参照获批并有相应有限真实React能力，其余状态与完整Native未齐、实际像素/有效render失败保留，产品仍必须blocked；合法元数据不能证明任意UI语义实际发生。

## 游戏、系统与资源原始证据

四款SR/MO/Never7/サクラノ詩必须来自同一最终包，各至少5个真实不同字幕状态，Never7至少3个可确认页。逐状态有稳定state/page身份、字幕hash、游戏/客户端配对及callback来源；两类截图hash各自在状态间唯一，复制同字节改名或任意不同图片均不能证明推进，真实adapter仍须核内容与归属。

OT-16按4引擎×4游戏展开16项，各自冻结engine/contract revision/backend/runtime及准确包/nonce，提供合法资源版本、文件hash、许可证据、每状态实际结果与独立质量审阅。SR既有中文版可记录真实非日语翻译、同语种直通或非日语降级；直通/未调用不记翻译质量。其他三款必须实际日语翻译及独立质量结论。资源未ready、无授权、unknown或route不支持继续blocked，不借其他引擎或游戏通过。4项Windows资格另用原机环境、同位数worker/DLL ABI、许可资源和独立Windows工件，不复用Mac包hash。

200%须实拍且证实整窗无裁切和全部动作可达；DOM存在或键盘入口可达不够。首次/拒绝/取消/撤销/恢复许可分别记录，拒绝不称live、不破坏独立Hook；Vault、导出与日志观察无明文秘密。真实SQLite重启、只读/磁盘满、迁移回滚及完整/过滤导出分别留证。

## 维护与证据边界

检查器分别维护于 `spec_contract.py/test_spec_contract.py`、`interaction_coverage.py`、存储验收工具、`current_product_coverage.py/test_current_product_coverage.py`及 `current_design_alignment.py/test_current_design_alignment.py`。真实负例覆盖缺变体、错层/错包、fixture替代、旧v4/原型冒当前产品、缺参照、少状态、伪命令/nonce、零断言/skip、日志冲突和证据篡改。临时合成registry/receipt仅为守卫测试，不进入生产表或产品结果。

hash与合法结构证明身份及内部一致性，不能独立鉴定任意文字声称的系统事件。完整发布须实际执行受信adapter、独立检查原始观察，并完成各业务层、原生、游戏、内容/视觉审阅和设计确认。失败原证据保留，修复后另建run；重建/重签包重新冻结身份及必需同包执行。原始私人文字、截图、路径和秘密仅受控保存，公开报告使用脱敏索引。

旧正文原字节保存在[文档归档](../archive/documentation-r75/docs/development/release-execution-evidence.md)，只供追溯，不作为现行执行入口。
