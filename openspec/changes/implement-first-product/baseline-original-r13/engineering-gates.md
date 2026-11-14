# OpenSpec、代码与编译的可执行门禁

本轮只建立研发设施，不实现产品功能。入口是根目录的 `npm run verify`，不是由协作者阅读文档后自行声明符合。失败返回非零，并在 `.build/quality/` 留下本次命令、源码指纹、退出码、测试数与日志。

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

门禁自测属于 `guard`，不是 `product-visual` 或 `product-runtime`。实际产品UI变更必须有已注册的真实产品截图捕获与对比检查；目前该adapter尚未实现，缺失时返回失败，不能拿材质案例自测顶替。

`validate_release`校验的是同包、用例完整性、证据路径及hash，不鉴定文档文字是否真实发生。完整发布还必须有真实产品运行检查adapter、确认的设计、零历史架构违规及同一正式包的必需用例。当前这些前置尚不具备，发布应拒绝；静态证据身份通过不能生成完整产品发布批准。

## 版本管理与CI

工程工具固定OpenSpec 1.2.0、Playwright Core 1.56.1、pixelmatch 7.1.0、pngjs 7.0.0，全部由根锁文件决定。产品依赖清单、锁及源代码不在本轮更改。

`.github/workflows/engineering-gates.yml`复用同一根命令并保留失败回执。当前目录尚无Git/remote，未初始化仓库、未提交推送，也未运行远端CI。进入正式仓库后需将该job设为必需检查并保护规则文件的审阅；只有工作流文件不能阻止远端绕过合并。

当前规范与技术锁变更遵循同一OpenSpec流程：先说明变更原因及受影响行为/模块/用例，再评审基线revision。不得自动修改基线、扩大债务、替换批准图或放宽阈值来让失败变绿。

## 首版分层测试登记门禁

[逐层测试规划](first-product-test-plan.md)以九层、128个定义及153个预冻结执行组合衔接B0～B5。[机器登记](first-product-test-plan.json)保留CR55/MC22/GL20原始来源，另展开31个机制验证；原111条CR不删除。`npm run verify:test-plan`校验来源与内容身份、合法引用、层依赖、变体真实性边界、覆盖与未执行状态；统一`npm run verify`另检查三份CSV导出与登记一致。

这些是静态计划/工程守卫，产品用例全部not_run。当前release仍使用97个平面来源ID做证据身份校验，**尚不能证明153组合已运行**；TE-10须随正式产品设施把`case@variant`完整清单接入真实运行/发布adapter，再允许完整发布。不能以本轮新增JSON或工具自测代替该接入。当前空adapter、19项旧债务和设计待审继续阻断。

## 数据库字段与存储模块的程序化门禁

新增入口：[存储验收规范](storage-acceptance.md)、[数据库字段与存储约束](../architecture/database-design.md#13-字段边界与存储模块的可执行验收)。`npm run verify:storage-contract`执行冻结schema比对、每字段边界实跑和子验收登记校验；统一verify同时运行对应正反例，检查实际probe回执ID/断言数/无skip，不只核对测试名称。

`storage_schema.py`比较实际SQLite结构与独立schema-contract，包括列/类型/空值/default/PK、STRICT、复合FK顺序与动作、CHECK、索引、触发器和额外对象。`storage_boundaries.py`将每字段及每CHECK绑定真实合法/非法写入和明确错误归因；逻辑冗余条件单列联合覆盖证书，未冻结容量政策不得擅填上限。哈希/u64/JSON的NUL截断绕过已在候选DDL补约束与独立语义负例。

`npm run verify:storage`是实际产品存储验收。执行器先验证当前冻结结构/边界登记，再实际调用代码注册的Rust编译与运行命令，读取本次断言和迁移候选库，以只读程序独立核对schema/来源/版本。缺adapter或PRE-14支持矩阵、旧源码/包/日志、缺路线/字段断言、skip/零断言、失败和结构漂移均非零拒绝。

开发态存储相关产品变更必须调用真实adapter；发布无条件要求。当前正式adapter为空，产品存储仍blocked。设计工具自身的fixture只证明守卫有效，不能把它注册成生产adapter或改写153产品组合not_run。字段投影有意隔离FK/业务trigger，不充当完整Repository、迁移或用户文件恢复验收。
