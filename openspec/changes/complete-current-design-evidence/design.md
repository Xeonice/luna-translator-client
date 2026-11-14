# Design

## Authority and boundaries
起点R84完整源486b13e2818b1d74272f44a874e26f1a6427faa223f99f6aa2ea472b85623de2。原61批准源、37历史图与原794状态不删除、不覆盖；新候选只能位于独立目录。源版批准、参照审核、真实组件、正式包/游戏执行分别记录。

## Reference catalogue
真实读取code-owned入口的catalogue和独立非作者审核，核target/composition/source closure、exact state/recipe/input/theme/viewport、原PNG与生成命令、审核覆盖集合/作者分离。缺项按状态报错，不自动批准、不接受任意JSON安装命令。部分能力不能代表全802。设计源变更须形成具体候选并获得确认，再以独立新版本关联。

## Execution
复用现有生产React宿主与固定构建、真实源map及像素重算。Native必须是当前source/资源/签名的真实正式包和实际进程/窗口/帧/交互，不接受controlled port、旧包或自报passed代替。通过代码登记精确state/layer/command与三个独立入口；旧v4只留历史责任和显式替代关系。完整公开行为/字段/技术变更不得借验证迁就现实现。

## Verification
负例覆盖未审核/错源/漏state/错recipe/篡改PNG/同作者/复用旧receipt/Native伪升级；源码/规范/技术/架构/目录、真实编译与原用例均保留。计划与运行回执分开，所有不可运行部分写blocked/not_run，不放宽零差异规则。

## Reference resolver v1 wire

Code-owned files are current-reference-catalogue.json and current-reference-review.json. The catalogue holds target/composition/61-source closure, a hash-bound archived generation pack and explicit published entries. Capture requests bind actual all-page row descriptor JSON (recipe, target, fixture and variant), not c.STATES alone. The generation pack contains immutable request/verification JSON, raw original target/full PNG, geometry, assertion transcript, actual command/nonce/time and before/after source hashes; only actual passed rows may become entries. A partially failing source replay stays failed in its raw report and does not grant the other states coverage. Independent review binds catalogue hash, exact approved entry digests, separate reviewer/author identities and explicitly grants reference-only approval. No captured PNG is approved by the generator. Original approval placeholders remain serialization-compatible; effective reference authority comes from this resolver, like effective source authority already comes from the independent source-version record. Product gates query the exact required state subset; release queries all states.

## Isolated packaged acceptance
显式验收profile只能在有效启动nonce下生成/验证系统临时目录，必须在任何runtime observation/DB/Vault/WindowCoordinator初始化前同时确定data与log根。私有profile UUID不能借用现有用户UUID；实际数据库恢复和Vault API保持。资源走当前bundle真实资源，目标锁保持canonical共享root，防止双实例各建锁而重复接入。正常启动忽略单独nonce，只有明确acceptance flag才隔离；不改变生产公共DTO、自动注入或模型行为。所有路径/符号链接/损坏marker/非法nonce负例须拒绝。


## Adopted C35 component supplement
用户明确采用PC-01～05，独立记录design-approval-preview-20261104.json绑定冻结5文件和父61源，保持captured pending标签不动。current_preview_authority校验该独立决策；它只授予C35源批准，不授予参照/产品/Native能力。涉及C35卡/详情或完整页面的父源recipe不得继续充当当前目标；独立阅读/正文等不含预览的已审裁片仍各保其范围。补充画布不直接替代完整main composition，逐状态recipe/参照及PC04历史元数据实现仍须单独完成。
