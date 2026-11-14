# 多行原文的可逆词法输出修复

- 需求：FP-04 / FP-07 / FP-10；组件 C16；Owner：native_core_impl。
- 授权：首版开发内的真实包缺陷修复；仅语言适配及其测试。UI 旧快照兼容由主流程独立负责。
- 允许路径：`crates/language-worker/src/lib.rs`、本变更记录；不修改资源清单、辞书、存储 schema 或用户数据，不重启运行中的产品或游戏进程。

## 已定位问题

MeCab 的 `rlength` 包含词面前被跳过的空白，`length` 仅为词面。原适配保留了每个词面的正确源跨度，却没有输出这些空白；持久化和 DTO 保留了这些合法间隔。UI 要求完整拼接等于源文本，因此多行文本整段退回纯文本。真实只读核对记录为 490 UTF-8 字节、168 UTF-16 单元、7 个 U+000A；115 个词法 token 各自范围和顺序正确，但拼接少了 7 字节，且无 whitespace token。

## 本次实现

- 将 MeCab 跳过的前置、中间和尾部空白作为独立、真实原文跨度输出；读音为空，不冒充词典已知词。
- 每个 gap 必须是合法 UTF-8 边界上的实际 Unicode 空白。非空白缺口或非法边界返回错误，不能伪造普通词。
- UTF-8 字节和 UTF-16 单元范围均须连续覆盖完整源文本，token 拼接必须逐字等于原文。
- JSONL 与 DTO 结构仍为现有版本；资源身份不变。新分析行为建议标记 `mecab-unidic-v2-lossless-whitespace`。现有持久化快照的分析版本、派生 ID 和旧记录保留由存储 Owner 处理，不在本文件中迁移或重写。

## 验证与完成条件

1. 真实随包 MeCab 0.996 / UniDic 2025.12：空格、tab、LF、CRLF、全角空白、前尾空白、全空白、日文混合 emoji。
2. 所有结果逐项核对 UTF-8/UTF-16 源跨度，完整拼接等于原文；已知日文词的真实读音保持可用。
3. 字面 `\\n` 与真实换行分别验证；非空白 gap / 非法边界负例必须拒绝。
4. 保留现有语言回归。执行真实 worker 测试，回执存入 `.build/`；未把此回归当作正式包和旧 UI 记录验收。
5. 旧词法记录不修改；旧 115-token 记录的空白兼容渲染与实际 HUD 验收由主流程执行。

## Root 集成与旧版本兼容

- Owner root；允许补充路径：`app-core/src/domain/capture.rs`、`app-core/src/ports/runtime.rs`、由生命周期 Owner 协作接入的 `application/runtime.rs`、`app-infra/src/language_services.rs`、`storage/queries/capture.rs`、`tests/storage_product.rs`、前端 `features/language/SourceText.tsx` 及 `tests/visual/verify-ui.mjs`（均在 crates/ 或 apps/desktop/ 既有范围内）。
- 语言端口显式提供分析行为版本；真实 MeCab 服务声明 v2，外部／测试适配器不冒称使用 MeCab。词法快照 ID 包含输出 schema 与分析版本，旧不可变记录不迁移、不覆盖。
- UI 对逐项 UTF-8/UTF-16 正确、有序且只缺空白的旧快照保留注音，空白按原文输出，不制造 token 索引；非空白缺口／错误范围仍退回纯原文。
- 增加实际存储版本共存/不可变测试，及 UI-31 的旧空白缺口、查词索引和非法范围负例。正式包另做真实旧记录恢复及新长文词法验证。
