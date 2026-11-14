# OpenSpec、代码与编译的可执行门禁

本文件说明研发门禁及其适用边界；产品实现与执行结果由具体OpenSpec工作项及当次机器回执记录。入口是根目录的 `npm run verify`，不是由协作者阅读文档后自行声明符合。失败返回非零，并在 `.build/quality/` 留下本次命令、源码指纹、退出码、测试数与日志。

## 使用

```sh
# 固定的工程工具；产品编译依赖仍使用apps/desktop自己的锁
npm ci --ignore-scripts --no-audit --no-fund
npm ci --prefix apps/desktop --no-audit --no-fund

# 本轮工程设施及实际TS/Rust编译检查
npm run verify -- --change enforce-spec-code-gates

# 仅门禁正反例，不代表产品符合
npm run test:guards

# 有Git时必须给真实base；CI禁止改用本地快照
npm run verify -- --change <change-id> --base <commit-sha>

# 完整产品发布校验；当前应被阻断
npm run verify:release -- --change <change-id> --artifact <package> --evidence <evidence.json>
```

首次Rust缓存不足时先 `cargo fetch --locked`；校验使用 `cargo check --workspace --locked --offline`，不会静默修改依赖锁。截图自测使用已安装的Chrome，或通过 `node node_modules/playwright-core/cli.js install chromium` 准备固定版本浏览器。

## 用户要求的五类故障

| 场景 | 真实故障注入与阻断方式 | 符合规范的对照 |
| --- | --- | --- |
| 1. PRD之外的内容 | 给现存模块/设计附上 `FP-99` / 未知产品ID；新增未授权cloud-sync文件；产品来源、反向场景映射或source-scope返回失败 | 已存在FP-03及其当前设计/模块/组件映射通过 |
| 2. 设计稿或实现与要求不一致 | 改材质/token源；引用旧稿/伪锚点；Playwright实际渲染改为亮粉色，与对照图pixelmatch差异超阈值时CLI退出1 | 同条件未改画面比较通过；批准源文件hash未变通过 |
| 3. 模块结构不一致 | 在实际fixture源码中跨feature私有导入、UI调用Tauri/HTTP；在Cargo加入或重命名/target隐藏非法依赖；AST/依赖守卫失败 | 公共index、端口、合法组合根和进程边界通过 |
| 4. 目录结构不一致 | 在未分类源码目录放文件，或在嵌套runtime/diagnostics/reports、符号链接中藏新增源码；目录/范围检查失败 | 已授权的目标路径与既有合法目录通过 |
| 5. 框架/技术结构不一致 | 修改真实package/Cargo引入Vue、Redux、Electron、Iced，替换Tauri或改变React/Rust版本；技术锁和语义决策检查失败 | 已锁React/TS/Tauri/Rust以及11份产品manifest/配置通过 |

测试使用临时源码和manifest执行实际检查，不修改产品。视觉故障仅修改浏览器当次渲染，原图不覆盖。自测图片保存在 `.build/quality/visual-adversarial/`。其中重放的材质案例用来证明比较器能拒绝差异，不是新产品窗口的视觉验收。

## 规范如何连接到代码

1. `openspec/specs/` 当前目标由FP表、MC和GL矩阵确定性生成；`product_specs.py` 默认只检查一致性，源PRD与生成规范不同即失败。12个FP、22个MC、20个GL身份和对应来源保留，生成不提升实现状态。
2. 每个change的 `verification.json` 将每个delta requirement/scenario双向映射到注册检查。产品项另需真实PRD ID、当前整稿的真实HTML锚点、M/C矩阵ID；未知项、漏项和宽泛授权均拒绝。
3. 每个注册检查ID绑定明确命令/测试类或文件，逐项执行并写同ID回执；工具不存在、测试文件删除、零测试、未知检查均不能通过。最后核对声明的检查确实执行。
4. source-scope用Git base的实际diff，包含新增/修改/删除；当前无Git用变更前快照。CI不接受快照替代base，也不把base当任意Git参数执行。
5. TypeScript AST分析与Cargo清单/本地依赖图分析检查规定层次。真实 `tsc --noEmit` 与Rust workspace编译另行执行；编译成功不能消除边界违规。
6. 旧PoC发现的19条问题逐条冻结，包含完整源文件SHA。无Git时债务账本受权威基线指纹约束；有Git时与base的账本比较，只能减少，不能自己增加或刷新旧hash。改旧文件而保留违规会失败，发布完全不接受旧问题。

工具不能单靠语法或编译理解任意新业务逻辑是否符合PRD。产品行为仍要绑定可执行场景测试；规则变更、测试质量与实际语义需要独立审阅。拥有规则修改权限者一起篡改规范与检查器不属于这些静态检查可以自行解决的问题。

## 实际产品视觉与发布保持拒绝缺项

门禁自测属于 `guard`，不是 `product-visual` 或 `product-runtime`。实际产品UI变更必须有已注册的真实产品截图捕获与对比检查；现已注册真实React组件的Playwright捕获/独立PNG复算adapter。补充同态对照须用户确认、独立技术审阅及新基线登记；缺参照、状态或任何差异都返回失败，不能拿材质案例自测顶替。浏览器对照不证明原生材质、窗口行为或游戏实测。

`validate_release`校验的是同包、用例完整性、证据路径及hash，不鉴定文档文字是否真实发生。完整发布还必须有真实产品运行检查adapter、确认的设计、零历史架构违规及同一正式包的必需用例。当前这些前置尚不具备，发布应拒绝；静态证据身份通过不能生成完整产品发布批准。

## 版本管理与CI

工程工具固定OpenSpec 1.2.0、Playwright Core 1.56.1、pixelmatch 7.1.0、pngjs 7.0.0，全部由根锁文件决定。产品依赖清单和源码的修改范围由对应OpenSpec迭代卡限制；本文件不自行扩大授权。

`.github/workflows/engineering-gates.yml`复用同一根命令并保留失败回执。当前目录尚无Git/remote，未初始化仓库、未提交推送，也未运行远端CI。进入正式仓库后需将该job设为必需检查并保护规则文件的审阅；只有工作流文件不能阻止远端绕过合并。

当前规范与技术锁变更遵循同一OpenSpec流程：先说明变更原因及受影响行为/模块/用例，再评审基线revision。不得自动修改基线、扩大债务、替换批准图或放宽阈值来让失败变绿。

## 首版分层测试登记门禁

[逐层测试规划](first-product-test-plan.md)以九层、128个定义及153个预冻结执行组合衔接B0～B5。[机器登记](first-product-test-plan.json)保留CR55/MC22/GL20原始来源，另展开31个机制验证；原111条CR不删除。`npm run verify:test-plan`校验来源与内容身份、合法引用、层依赖、变体真实性边界、覆盖与未执行状态；统一`npm run verify`另检查三份CSV导出与登记一致。

原2026-10-18/19登记属于静态计划，原用例状态保持not_run；当时97来源尚不足以证明153组合已执行。当前 `spec_contract` 已强制验证完整153逐组合回执，并有有限SR6正式包、真实存储/组件等执行器；这些事实不证明当前完整153或最新guided附加组合已通过。旧“空adapter/19项债务”只属原规划时快照，不作为当前现状。当前批准范围、债务和执行结果按实际源码/当次回执分别判断；新576组合及其设计pending按本文件末节独立阻断。

## 数据库字段与存储模块的程序化门禁

新增入口：[存储验收规范](storage-acceptance.md)、[数据库字段与存储约束](../architecture/database-design.md#13-字段边界与存储模块的可执行验收)。`npm run verify:storage-contract`执行冻结schema比对、每字段边界实跑和子验收登记校验；统一verify同时运行对应正反例，检查实际probe回执ID/断言数/无skip，不只核对测试名称。

`storage_schema.py`比较实际SQLite结构与独立schema-contract，包括列/类型/空值/default/PK、STRICT、复合FK顺序与动作、CHECK、索引、触发器和额外对象。`storage_boundaries.py`将每字段及每CHECK绑定真实合法/非法写入和明确错误归因；逻辑冗余条件单列联合覆盖证书，未冻结容量政策不得擅填上限。哈希/u64/JSON的NUL截断绕过已在候选DDL补约束与独立语义负例。

`npm run verify:storage`是实际产品存储验收。执行器先验证当前冻结结构/边界登记，再实际调用代码注册的Rust编译与运行命令，读取本次断言和迁移候选库，以只读程序独立核对schema/来源/版本。缺adapter或PRE-14支持矩阵、旧源码/包/日志、缺路线/字段断言、skip/零断言、失败和结构漂移均非零拒绝。

开发态存储相关产品变更必须调用真实adapter；发布无条件要求。现已注册`rust-storage-complete-v1`实际生产adapter（600秒编译、1800秒全量运行预算），覆盖22项、431字段、完整边界及十路线；必须实际执行并核对当前源码和真实包，不能仅因注册解除阻断。设计工具自身的fixture只证明守卫有效，不能把它注册成生产adapter或改写153产品组合not_run。字段投影有意隔离FK/业务trigger，不充当完整Repository、迁移或用户文件恢复验收。

## 2026-10-19 两项验收工作项

`implement-first-product/acceptance-closure-work-item.md`落实同态视觉与完整存储验收。原设计/原PNG/token/零差异阈值保留；补充参照登记在独立acceptance-v3目录，实际客户端输出与参照分开，所有登记的滚动变体均须复算。原生系统控制和材质单独验收。新包身份包含签名保护的源码记录、实际EXE及资源hash；拒绝仅重写外部JSON的旧包伪装。

代码、编译和adapter守卫通过不等于完整首版发布；153组合、原生窗口、四游戏和真实模型等仍按最终同包回执核对。历史47/49及旧包四游戏结果保留，不能继承到新候选。

## 2026-10-19：补齐已评审设计的执行覆盖

新增[RW逐项逐层测试合同](reading-window-test-coverage.md)与[发布执行证据格式](release-execution-evidence.md)。34项RW从权威CSV闭包检查，按设计/真实产品组件/真实数据管线/真实语言/原生/游戏六层划分；缺层、漏状态、旧源码/包、零断言、无nonce、截图未关联、跨层冒充均应失败。设计adapter由`interaction-design-execution`实际启动，`interaction-coverage-contract`与`interaction-coverage-tests`校验登记及正反例；它们不属于产品运行adapter。原计划CSV不重写，153个case@variant执行证据单独验证。

设计层所需已有本机Chrome与固定Playwright；真实层所需正式包、已核对Core/词典资源、可用桌面/权限、多屏与快捷键冲突条件、真实回调和可推进对白的游戏，继续由原测试供给矩阵维护。新层adapter尚未实现，不能用设计样例补齐。测试源码归`scripts/quality/interaction_coverage.py`与`test_interaction_coverage.py`、设计动作归`scripts/report/verify_interaction_gap_design.cjs`、映射归`reading-window-test-coverage.json`，由QA主责、UI/AP/PL/HK/JA按层协作。每次执行写独立`.build/quality/`回执，失败保留；历史成功不追认新范围。


## 2026-10-30：模型服务设计的同源文档检查

`specify-full-provider-registry-design`为tooling文档工作项，未授权产品协议、DDL、界面或运行包改动。新登记`model-service-design-docs`实际执行`python3 scripts/report/build_model_service_design_report.py --check`，校验13协议/21接口预设、用户明确排除TMT、精确引用与用途子集、新验收not_run、主技术稿两张生成矩阵和机器目录一致。`model-service-design-docs-tests`实际执行独立正反例，覆盖缺项、悬空引用、TMT回流、伪造通过及导出漂移；仅为文档工具守卫，不是生产协议测试。

HTML导出复用已安装Markdown工具，指定新的`--output reports/<new-report>`；输出源文件hash与manifest并拒绝覆盖旧报告。文档/HTML检查、scope/基线检查和统一verify分别留回执；即使文档工作项完整verify通过，也不提升任何新协议、迁移、OAuth、UI、游戏或发布验收。新协议实现必须另建product工作项并接入真实fixture/系统存储/API/同包验收。

## 本机离线翻译设计增量

工作项design-local-offline-translation新增 `offline-translation-design` 与 `offline-translation-design-tests`：对当前离线目录/PRD/OT计划/独立整稿依赖执行静态核对、真实Playwright正常与异常旅程、截图及负例。新稿在first-product-v1/candidates/local-offline-20261030，pending评审；canonical批准源、PNG、technology-lock与历史11alias原字节保留。来源变更、页面异常、状态错配或断言失败均拒绝，不能用guard通过提升实际系统API/DLL/断网/四游戏或字段迁移结果。维护与前置见[新测试计划](local-offline-translation-tests.md)。

## 2026-10-31 当前guided目标的附加覆盖

最新PRD/客户端目标由[current-product-test-coverage.json](current-product-test-coverage.json)追加追溯，保留旧128定义/153组合/97来源/111行CSV及62个PA/OT/SS/FL/GD原记录。70来源还包含现有guided设计runner真实GUI01～08回归；576观察组合覆盖精确25服务/13协议/4引擎、权限/Vault/SQLite/200%及同包四游戏。该登记不是可执行产品实现或设计批准。

统一入口无条件运行 `current-product-coverage-contract`（`validate_registry`）；release另无条件运行 `current-product-release-coverage`（`release_coverage_gaps`），不由本change requested checks开关决定。原153仍由 `spec_contract.validate_release`独立执行，新附加ID走原release证据的 `current_product_coverage` path/hash侧录，不塞入旧执行集合。维护入口为 `scripts/quality/current_product_coverage.py` 与 `test_current_product_coverage.py`。

```sh
python3 scripts/quality/current_product_coverage.py --check --json
python3 scripts/quality/current_product_coverage.py --assess --evidence .build/<real-run>/release.json --artifact dist/<actual-artifact> --json
```

首条只校验计划/来源/元数据；后条只审查当前真实证据并给准出缺口，不替用户运行产品。当前新范围生产adapter表为空，后条缺证据/能力必须blocked且exit1。已登记SR6正式包稳定性runner、旧组件/存储/截图adapter仍保留各自范围；它们没有自动获得新576组合或完整153的能力。fixture adapter仅限临时守卫测试，不注册到生产表。

守卫必须实际拒绝来源/单服务/协议/引擎遗漏、tuple或版本漂移、必需层/OwnerM-C/前置削弱、旧源错包、伪command/nonce、缺日志、零断言/skip和跨层冒充；另外拒绝重复事件冒五字幕、Never7少页、200%仅有DOM但裁切、权限取消当live、Vault证据未核秘密边界。静态pass、守卫pass与准出blocked分别报告，不据工程工具解除产品/设计/发布审批。

当前guided完整候选的 `pending_review` 也是 `release_coverage_gaps` 的独立准出阻断。即使单项执行回执完整，也只能解除该项执行缺口；不能由adapter、JSON或历史整稿批准授予当前设计确认。未来独立确认登记须经评审更新相应合同，不能改本计划状态或消除其他缺项来绕过。

当前补充准出仅覆盖既定macOS arm64+CrossOver同一包：576必需观察；原OT-08/OT-16的J北京/金山Windows原机4项另列 `separate_platform_qualifications`，保持not_run、独立环境/ABI/合法资源/Windows工件，不能当Mac包通过或从目录删掉。机器代码限定这4项单列，不允许JSON任意缩减当前scope；未来Windows发布需独立范围与真实准出。

OT-16中SR既有中文版只按真实同语种原文、直通、非日语降级或实际非日语翻译分别记录；直通/未调用引擎的证据只能通过非日语分支检查，不得记日语译文质量。其他三款日文游戏必须有该引擎真实调用、原译文及独立日语质量结论。每引擎/游戏的资源状态必须ready、已授权且route支持；缺授权/未知/不支持仍blocked，不因直通、显示中文或其他引擎通过而绕过。
