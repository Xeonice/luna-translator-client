# 当前guided目标：逐来源测试与附加准出

现行门禁采用[九类统一检查](engineering-gates.md)，视觉G09为正式人工确认门禁；门禁有效性与产品验收分开，实际执行统一查看[结果页](../../reports/gates/index.html)。原产品功能、状态、存储和四游戏义务不减。
当前guided完整源版本已由用户明确审核通过，身份见[源版批准记录](design-approval-guided-20261031.json)；其他页面按[全页与状态闭包](all-page-design-coverage.md)逐项对齐。设计源批准、独立参照、真实组件和正式包验收分别记录；原29个阅读/配置增量状态继续保留，不能代表整套页面。

当前目标为[guided当前阅读](../design/first-product-v1/candidates/guided-service-reading-20261030/index.html?scenario=reading)和[必要配置](../design/first-product-v1/candidates/guided-service-reading-20261030/index.html?scenario=settings)：当前句独立首屏，历史/全部提取/Hook后置默认折叠，用途→品牌→明确接口/引擎→必要字段配置。完整候选源版本已获用户审核确认，文档和设计演示不追认产品对齐。

[current-product-test-coverage.json](current-product-test-coverage.json)保留PA12+OT16+SS12+FL10+GD12原62记录及GUI8设计回归来源，共70来源，按适用层/环境展开576个追加观察 `case@layer@variant`。25服务=21HTTP预设+4离线引擎，HTTP协议13种；精确tuple、版本与用途不可因呈现简化更改。不是全笛卡尔积，普通凭据不要求为每provider购买两把Key。

该集合累计于原111条CR定义、128定义/153执行组合、97来源、RW34六层和真实存储专项之外，均独立强制。当前设计另有[完整页面状态/61源组合对齐合同](current-design-alignment.json)，不能用576业务观察替代。当前scope固定 `macos-arm64-crossover`；OT-08/OT-16×J北京/金山仅4项Windows原机资格另列 `separate_platform_qualifications`，原样not_run，不计入Mac同包576项，不允许把其他必需观察移出scope。

每条机器记录保留精确原断言、需求、设计锚点、Owner/M-C、必需层、前置、步骤和可观察期望。下表为人工索引；全部产品计划 `not_run`，真实运行另记，外部条件不足为blocked。GD为权威验收ID，与设计runner的GD旅程编号语义不同，不按编号相同推断覆盖。

| 来源 | 原目标/可观察行为 | 需求 | Owner；模块；组件 | 必需层 | 前置 |
|---|---|---|---|---|---|
| PA-01 | 注册完整性：重复 ID、预设引用缺失适配器、目录/实现不一致都拒绝启动。 | FP-06, FP-08, FP-11, FP-12 | AI,AP,QA,UI；M04,M05,M09,M13；C22,C26,C27,C29,C31 | catalog, domain | PRE-01, PRE-13 |
| PA-02 | 所有入口精确查找：未知协议在应用、发送、测试、发现四处拒绝；零取密钥、零网络；禁止默认 Bearer / GET models。 | FP-06, FP-08, FP-11, FP-12 | AI,AP,PL,QA,UI；M04,M05,M06,M09,M13,M16；C22,C26,C27,C29,C30,C31 | credentials, domain, protocol | PRE-01, PRE-13, PRE-14 |
| PA-03 | 用途与硬能力：传统翻译用于 grammar/context_word 必须失败；用户声明不能突破 adapter 硬限制。 | FP-06, FP-08, FP-11, FP-12 | AI,AP,QA,UI；M04,M05,M06,M09,M13；C22,C26,C27,C29,C31 | domain, protocol | PRE-01, PRE-13 |
| PA-04 | 请求契约：本地 HTTP fixture 实际检查路由、认证、签名、语言码、模型与参数白名单。 | FP-06, FP-08, FP-11, FP-12 | AI,AP,QA,UI；M04,M05,M06,M09,M13；C22,C26,C27,C29,C31 | protocol, service | CURRENT-SERVICE-MATERIALS, PRE-01, PRE-06, PRE-11, PRE-13 |
| PA-05 | 流与终止：拆分 UTF-8 / SSE、缺终止、流内错误、拒绝、截断、reasoning-only / tool 输出、超大响应。 | FP-06, FP-08, FP-11, FP-12 | AI,AP,QA,UI；M04,M05,M06,M09,M13；C22,C26,C27,C29,C31 | protocol, service | CURRENT-SERVICE-MATERIALS, PRE-01, PRE-06, PRE-11, PRE-13 |
| PA-06 | 生命周期隔离：取消、撤权、停用、换配置后迟到流不能写成功/缓存；future 不得脱离任务。 | FP-06, FP-08, FP-11, FP-12 | AI,AP,QA,UI；M04,M05,M06,M09,M10,M13；C22,C26,C27,C29,C31 | domain, service, storage | CURRENT-SERVICE-MATERIALS, PRE-01, PRE-06, PRE-11, PRE-13, PRE-14 |
| PA-07 | 秘密和网络隔离：Header/query/body/错误/Debug 脱敏；共用 client 不串 Key，Key 不进入默认头或普通 JSON。 | FP-06, FP-08, FP-11, FP-12 | AI,AP,PL,QA,UI；M04,M05,M06,M09,M13,M16；C22,C26,C27,C29,C30,C31 | credentials, protocol | PRE-01, PRE-13, PRE-14 |
| PA-08 | 旧配置迁移：真实旧 profile 备份副本迁移、事务回滚、FK/seal/历史保持；旧客户端明确拒绝新 schema。 | FP-06, FP-08, FP-11, FP-12 | AI,AP,QA,UI；M04,M05,M09,M10,M13；C22,C26,C27,C29,C31 | storage | PRE-01, PRE-13, PRE-14 |
| PA-09 | 身份与缓存：从已保存adapterBinding冻结protocol/contract/profile版本进入请求/测试/缓存身份；升级失效，preset更新不能重解释旧配置。 | FP-06, FP-08, FP-11, FP-12 | AI,AP,QA,UI；M04,M05,M09,M10,M13；C22,C26,C27,C29,C31 | domain, storage | PRE-01, PRE-13, PRE-14 |
| PA-10 | 架构与 UI：UI/核心不得绕层 HTTP/SQL/FFI；目录驱动表单保持草稿、测试、应用和启用分离。 | FP-06, FP-08, FP-11, FP-12 | AI,AP,QA,UI；M04,M05,M09,M13；C22,C26,C27,C29,C31 | component, domain | PRE-01, PRE-13 |
| PA-11 | 真实服务验收：每服务实际 ja→zh、多句连续、取消/失败恢复；学习用途单独核解释结果，模型列表成功不代替。 | FP-06, FP-08, FP-11, FP-12 | AI,AP,QA,UI；M04,M05,M06,M09,M13；C22,C26,C27,C29,C31 | service | CURRENT-SERVICE-MATERIALS, PRE-01, PRE-06, PRE-11, PRE-13 |
| PA-12 | 同包游戏回归：新旧候选分别固定包身份；所有新服务改动在同一新包内完成 SR/MO/Never7/樱之诗回归，不继承旧包通过。 | FP-06, FP-08, FP-11, FP-12 | AI,AP,PL,QA,UI；M04,M05,M09,M12,M13；C22,C26,C27,C29,C31,C34,C35 | game | PRE-01, PRE-07, PRE-08, PRE-10, PRE-12, PRE-13, PRE-17 |
| OT-01 | 目录与执行方式：HTTP13协议/21预设逐对象不变，离线目录与http/platform_native/local_worker分开。 | AI-CFG-01, AI-CFG-14, FP-06 | AI,AP,QA,UI；M04,M05,M13；C26,C27,C29,C31 | catalog | PRE-01, PRE-13 |
| OT-02 | 离线条件字段：四引擎按实际类型展示字段，没有假URL/模型ID/Key或虚假模型实体。 | AI-CFG-01, AI-CFG-02, AI-CFG-08, FP-06, FP-11 | AI,AP,QA,UI；M04,M05,M13；C26,C27,C29,C31 | component, design | PRE-01, PRE-13 |
| OT-03 | 用途硬限制：离线仅translation，grammar/context_word在应用/测试/执行前拒绝，零取凭据/零外发。 | AI-CFG-03, AI-CFG-06, FP-06, FP-08 | AI,AP,QA,UI；M04,M05,M09,M13；C22,C26,C27,C29,C31 | component, domain | PRE-01, PRE-13 |
| OT-04 | 配置生命周期：草稿/本机探测/真实样句测试/应用/启用独立；CAS冲突、测试过时与恢复可见。 | AI-CFG-04, AI-CFG-05, AI-CFG-13, FP-06 | AI,AP,QA,UI；M04,M05,M13；C26,C27,C29,C31 | component, domain | PRE-01, PRE-13 |
| OT-05 | Apple版本与许可：按OS/SDK符号可用性、传统策略和同策略语言对核对；系统下载授权拒绝/取消不提升就绪。 | AI-CFG-06, AI-CFG-14, FP-06, FP-11 | AI,AP,PL,QA,UI；M04,M05,M12,M13；C26,C27,C29,C31,C34,C35 | native | PRE-01, PRE-08, PRE-10, PRE-13 |
| OT-06 | 资源缺失与失效：未安装、缺包、损坏、版本不兼容、路径移动各有真实状态和恢复，不以目录存在判成功。 | AI-CFG-05, AI-CFG-14, FP-11, FP-12 | AI,AP,PL,QA,UI；M04,M05,M06,M12,M13,M15；C26,C27,C29,C31,C32,C33 | domain, worker | CURRENT-OFFLINE-LICENSE, PRE-01, PRE-11, PRE-13 |
| OT-07 | 直达与显式中转：默认direct_only，缺直达包不自动pivot；明确应用完整路径和逐跳资源，失败不换路由。 | AI-CFG-09, AI-CFG-12, FP-06 | AI,AP,PL,QA,UI；M04,M05,M06,M12,M13,M15；C26,C27,C29,C31,C32,C33 | domain, worker | CURRENT-OFFLINE-LICENSE, PRE-01, PRE-11, PRE-13 |
| OT-08 | DLL进程与平台隔离：J北京/金山仅同位数独立worker加载；不入UI/主业务/Hook/游戏；Windows与CrossOver独立验证。 | AI-CFG-14, FP-06, FP-12 | AI,AP,PL,QA,UI；M04,M05,M06,M12,M13,M15；C26,C27,C29,C31,C32,C33 | worker | CURRENT-OFFLINE-LICENSE, PRE-01, PRE-11, PRE-13 |
| OT-09 | worker故障与预算：卡死/崩溃/超时/超长输出及队列资源均有界；只回收自有worker，不阻塞Hook/UI/词法。 | AI-CFG-12, FP-04, FP-12 | AI,AP,PL,QA,UI；M04,M05,M06,M12,M13,M15；C26,C27,C29,C31,C32,C33 | protocol, worker | CURRENT-OFFLINE-LICENSE, PRE-01, PRE-11, PRE-13 |
| OT-10 | 取消与迟到：取消/停用/换配置/换资源推进终态或epoch；旧worker/旧attempt迟到不写成功/缓存，原句归属保持。 | AI-CFG-13, FP-04, FP-09 | AI,AP,QA,UI；M04,M05,M10,M13；C26,C27,C29,C31 | domain, storage | PRE-01, PRE-13, PRE-14 |
| OT-11 | 断网真实翻译：指定真实安装/语言包在断网环境产生实际译文；不隐式联网、云回退、后台下载。 | AI-CFG-04, AI-CFG-12, AI-CFG-14, FP-06, FP-10 | AI,AP,PL,QA,UI；M04,M05,M06,M12,M13,M15；C26,C27,C29,C31,C32,C33 | worker | CURRENT-OFFLINE-LICENSE, PRE-01, PRE-11, PRE-13 |
| OT-12 | 资源版本与缓存：engine/backend/worker/包/route/config进入测试与缓存身份；升级/观察代次变化不误命中。 | AI-CFG-05, AI-CFG-13, FP-09, FP-11 | AI,AP,QA,UI；M04,M05,M10,M13；C26,C27,C29,C31 | domain, storage | PRE-01, PRE-13, PRE-14 |
| OT-13 | codec与真实迁移：新旧codec/seal保真、EngineTarget XOR和同版本FK、未知版本拒绝、GC与真实迁移/回滚。 | AI-CFG-15, FP-06, FP-12 | AI,AP,QA,UI；M04,M05,M10,M13；C26,C27,C29,C31 | storage | PRE-01, PRE-13, PRE-14 |
| OT-14 | 导入与本机重绑定：导出不带DLL/商业资源/绝对安装路径/授权/测试通过；导入和复制重新定位并明确应用。 | AI-CFG-10, AI-CFG-15, FP-06, FP-11 | AI,AP,QA,UI；M04,M05,M10,M13；C26,C27,C29,C31 | component, storage | PRE-01, PRE-13, PRE-14 |
| OT-15 | 多引擎与历史：有序独立结果、全局/游戏覆盖、停用、人工修正和历史来源保持，不自动HTTP回退。 | AI-CFG-03, AI-CFG-10, AI-CFG-13, FP-04, FP-09 | AI,AP,QA,UI；M04,M05,M10,M13；C26,C27,C29,C31 | component, domain, storage | PRE-01, PRE-13, PRE-14 |
| OT-16 | 正式包与语言质量：同一最终包逐engine/平台/合法资源核实际离线译文、多句/Unicode/简繁质量与四游戏；不继承旧结果。 | AI-CFG-05, AI-CFG-06, AI-CFG-14, FP-04, FP-06, FP-10, FP-12 | AI,AP,PL,QA,UI；M04,M05,M06,M12,M13,M15；C26,C27,C29,C31,C32,C33,C34,C35 | game, review, worker | CURRENT-GUI-REVIEW, CURRENT-OFFLINE-LICENSE, PRE-01, PRE-07, PRE-08, PRE-10, PRE-11, PRE-12, PRE-13, PRE-17 |
| SS-01 | 同一浅层选择器：当前会话浅层与HUD复用同一翻译服务选择器、同一已保存列表及有效绑定投影。 | AI-CFG-16, FP-04, FP-06 | AI,AP,PL,QA,UI；M04,M05,M12,M13；C26,C27,C29,C31,C34,C35 | component, design, native | PRE-01, PRE-08, PRE-10, PRE-13 |
| SS-02 | 仅已保存且用途支持：只允许当前配置档已添加、已保存、字段合法且显式支持translation的资源；目录模板只在添加服务，未保存草稿不成为可选项。 | AI-CFG-01, AI-CFG-04, FP-06, FP-11 | AI,AP,QA,UI；M04,M05,M13；C26,C27,C29,C31 | design, domain | PRE-01, PRE-13 |
| SS-03 | 禁选原因与恢复：未配置、缺字段/凭据、已停用及当前版本已知失败不可选，并显示对应对象/版本的原因和修复入口。 | AI-CFG-04, AI-CFG-05, AI-CFG-11, FP-06, FP-12 | AI,AP,PL,QA,UI；M04,M05,M06,M13,M16；C26,C27,C29,C30,C31 | credentials, design, domain, service | CURRENT-SERVICE-MATERIALS, PRE-01, PRE-06, PRE-11, PRE-13, PRE-14 |
| SS-04 | HTTP未验证不等于未配置：HTTP合法保存且用途支持但未测时可明确选用并保持未验证；新版本不挪用旧测试，未测试不导致永久禁选。 | AI-CFG-04, AI-CFG-05, FP-06 | AI,AP,QA,UI；M04,M05,M06,M13；C26,C27,C29,C31 | design, domain, service | CURRENT-SERVICE-MATERIALS, PRE-01, PRE-06, PRE-11, PRE-13 |
| SS-05 | 本机健康门禁：本机仍按系统能力、资源、语言对与运行route门禁判定；必要条件未知、缺包或未实现路线不可选。 | AI-CFG-14, FP-06, FP-11, FP-12 | AI,AP,PL,QA,UI；M04,M05,M06,M12,M13,M15；C26,C27,C29,C31,C32,C33,C34,C35 | design, native, worker | CURRENT-OFFLINE-LICENSE, PRE-01, PRE-08, PRE-10, PRE-11, PRE-13 |
| SS-06 | 显式用途与能力过滤：翻译列表按templatePurpose和translation能力筛选，精读-only不列入，不用名称/品牌推断用途，不升级离线能力。 | AI-CFG-03, AI-CFG-06, AI-CFG-07, FP-06, FP-08 | AI,AP,QA,UI；M04,M05,M09,M13；C22,C26,C27,C29,C31 | catalog, design, domain | PRE-01, PRE-13 |
| SS-07 | 保存与选用分离：新增编辑保存或指定样句测试不改变current服务；明确选择仅引用已保存不可变revision及其冻结依赖。 | AI-CFG-04, AI-CFG-10, FP-06 | AI,AP,QA,UI；M04,M05,M10,M13；C26,C27,C29,C31 | design, domain, storage | PRE-01, PRE-13, PRE-14 |
| SS-08 | 选中后续句与独立启停：选择即activate新绑定并只作用后续句，保持purpose enabled/会话开关；当前句重译独立且历史旧结果保留。 | AI-CFG-03, AI-CFG-04, AI-CFG-13, FP-04, FP-06, FP-09 | AI,AP,PL,QA,UI；M04,M05,M06,M12,M13；C26,C27,C29,C31,C34,C35 | design, domain, native, service | CURRENT-SERVICE-MATERIALS, PRE-01, PRE-06, PRE-08, PRE-10, PRE-11, PRE-13 |
| SS-09 | 全局与游戏继承：明确全局默认、游戏覆盖与恢复继承来源；修改不串游戏或配置档，取消不改变任何绑定。 | AI-CFG-10, FP-06, FP-11 | AI,AP,QA,UI；M04,M05,M10,M13；C26,C27,C29,C31 | design, domain, storage | PRE-01, PRE-13, PRE-14 |
| SS-10 | 取消与迟到隔离：取消选择不改绑定；新绑定取消旧队列/在途，旧分片不覆盖当前句或写新成功/缓存，既有历史保留原来源。 | AI-CFG-12, AI-CFG-13, FP-04, FP-09, FP-12 | AI,AP,QA,UI；M04,M05,M06,M10,M13；C26,C27,C29,C31 | design, domain, service, storage | CURRENT-SERVICE-MATERIALS, PRE-01, PRE-06, PRE-11, PRE-13, PRE-14 |
| SS-11 | 主对照有序去重：主翻译仅一项、对照有序；同一资源不同时作为主与对照，同名不同配置按ID区分，单项失败不取消其他结果。 | AI-CFG-03, AI-CFG-12, FP-04, FP-06 | AI,AP,PL,QA,UI；M04,M05,M12,M13；C26,C27,C29,C31,C34,C35 | design, domain, native | PRE-01, PRE-08, PRE-10, PRE-13 |
| SS-12 | 带上下文配置往返：去配置携带来源/scope/game/purpose/目标并可返回原选择器；保存或取消都不自动选用、测试、发送或重译。 | AI-CFG-16, FP-06, FP-08, FP-11 | AI,AP,PL,QA,UI；M04,M05,M09,M12,M13；C22,C26,C27,C29,C31,C34,C35 | component, design, native | PRE-01, PRE-08, PRE-10, PRE-13 |
| FL-01 | 统一服务管理与同页编辑：同一已添加服务列表同时呈现云端/本机非大模型/LLM资源，类型为标签；选中后同页编辑，无family和五实体Tab导航。 | AI-CFG-01, AI-CFG-02, AI-CFG-16, FP-06, FP-11 | AI,AP,QA,UI；M04,M05,M13；C26,C27,C29,C31 | component, design, domain | PRE-01, PRE-13 |
| FL-02 | 条件字段与真实对象：传统HTTP、LLM和离线引擎仅展示适用字段；不虚造模型ID/URL/Key，不通过隐藏必要未知条件绕过门禁。 | AI-CFG-01, AI-CFG-02, AI-CFG-06, AI-CFG-14, FP-06, FP-11 | AI,AP,PL,QA,UI；M04,M05,M12,M13；C26,C27,C29,C31,C34,C35 | catalog, component, design, native | PRE-01, PRE-08, PRE-10, PRE-13 |
| FL-03 | 资源版本与配置选择分离：连接/模型/用途和独立离线资源关系不变；草稿/保存/检查/样句不改当前绑定，明确选择、应用和启停保持原语义。 | AI-CFG-03, AI-CFG-04, AI-CFG-05, AI-CFG-10, AI-CFG-13, AI-CFG-16, FP-06, FP-11 | AI,AP,QA,UI；M04,M05,M06,M10,M13；C26,C27,C29,C31 | design, domain, service, storage | CURRENT-SERVICE-MATERIALS, PRE-01, PRE-06, PRE-11, PRE-13, PRE-14 |
| FL-04 | 高级能力与学习用途保留：适用模板/参数/上下文/运行策略/密钥池可按需展开并有摘要；精读和语境查词独立配置，互不借用或自动启用。 | AI-CFG-03, AI-CFG-07, AI-CFG-08, AI-CFG-09, AI-CFG-11, AI-CFG-12, FP-06, FP-08, FP-11 | AI,AP,PL,QA,UI；M04,M05,M09,M13,M16；C22,C26,C27,C29,C30,C31 | component, credentials, design, domain | PRE-01, PRE-13, PRE-14 |
| FL-05 | 会话三独立区域同页可达：正文历史、原始提取和Hook诊断同页保留各区标题/范围/计数/摘要/入口，详情按需展开，不互斥替换。 | FP-09, FP-12, RW-17, RW-18, RW-31, RW-32 | AP,PL,QA,UI；M03,M10,M12,M13,M15；C16,C17,C18,C34,C35,C36 | component, design, native | PRE-01, PRE-08, PRE-10, PRE-13 |
| FL-06 | 区域视图状态互不污染：各区查询/筛选/页/滚动独立；操作一处不重置另一处，不切流、不重注入、不暂停取文。 | FP-09, FP-12, RW-19, RW-21, RW-23 | AP,QA,UI；M03,M10,M13,M15；C16,C17,C18,C36 | component, design, domain, storage | PRE-01, PRE-13, PRE-14 |
| FL-07 | 全量数据搜索与导出保留：选流前和未选流事件、顺序/身份/增量关系、脱敏Hook原生事实、缺口、全量分页/搜索及明确数据集导出完整保留。 | FP-09, FP-12, RW-17, RW-18, RW-19, RW-20, RW-22, RW-23 | AP,HK,PL,QA,UI；M02,M03,M10,M12,M13,M14,M15；C16,C17,C18,C33,C34,C35,C36 | design, native, pipeline, storage | PRE-01, PRE-08, PRE-10, PRE-13, PRE-14 |
| FL-08 | 独立跟随与常驻预览：各区独立跟随/暂停和新到计数；上翻某区仍收取事件，折叠/滚动不重启同一会话预览或改变采集时间身份。 | FP-03, FP-09, FP-12, RW-01, RW-02, RW-05, RW-21 | AP,HK,PL,QA,UI；M02,M03,M10,M12,M13,M14,M15；C16,C17,C18,C33,C34,C35,C36 | component, design, native, pipeline | PRE-01, PRE-08, PRE-10, PRE-13 |
| FL-09 | 功能审计与演示控制归位：清理重复导航/日常选用入口和不适用字段；演示返回/故障注入只在页外工具，不留空按钮、不伪造成功、不删除高级与审计能力。 | AI-CFG-11, AI-CFG-12, AI-CFG-13, AI-CFG-16, FP-06, FP-09, FP-11, FP-12, RW-17, RW-18, RW-34 | AI,AP,QA,UI；M03,M04,M05,M10,M13,M15；C16,C17,C18,C26,C27,C29,C36 | component, design, review | CURRENT-GUI-REVIEW, PRE-01, PRE-13 |
| FL-10 | 可访问布局与证据隔离：键盘/焦点、暗浅色、窄窗、长标签和200%下同页区域仍可达；新候选与截图独立留存，旧候选/目录/计划/批准源及产品状态不变。 | FP-06, FP-09, FP-11, FP-12, RW-31, RW-32, RW-33, RW-34 | AI,AP,PL,QA,UI；M03,M04,M05,M10,M12,M13,M15；C16,C17,C18,C26,C27,C29,C34,C35,C36 | component, design, native | PRE-01, PRE-08, PRE-10, PRE-13 |
| GD-01 | 独立当前正文优先：当前原文、译文/状态及来源是首屏独立阅读卡，常用动作可达，不以整份历史列表替代当前对象。 | AI-CFG-13, FP-04, FP-06, FP-09, RW-17, RW-31, RW-32 | AI,AP,PL,QA,UI；M03,M04,M05,M10,M12,M13,M15；C16,C17,C18,C26,C27,C29,C34,C35,C36 | component, design, domain, native | PRE-01, PRE-08, PRE-10, PRE-13 |
| GD-02 | 历史与诊断后置默认折叠：正文历史、全部提取、Hook诊断在当前阅读后方同页各自默认折叠，数量和警告/错误/缺口摘要保持可见。 | FP-04, FP-09, FP-12, RW-17, RW-18, RW-20, RW-21 | AP,HK,QA,UI；M02,M03,M10,M13,M14,M15；C16,C17,C18,C33,C36 | component, design, pipeline | PRE-01, PRE-13 |
| GD-03 | 折叠保持全量与独立状态：折叠/展开不重置查询/筛选/页/滚动/跟随/新到计数，不丢数据/停止订阅或重建预览，完整与过滤导出范围保持。 | FP-03, FP-09, FP-12, RW-01, RW-17, RW-18, RW-19, RW-20, RW-21, RW-22, RW-23 | AP,HK,PL,QA,UI；M02,M03,M10,M12,M13,M14,M15；C16,C17,C18,C33,C34,C35,C36 | component, design, native, pipeline, storage | PRE-01, PRE-08, PRE-10, PRE-13, PRE-14 |
| GD-04 | 品牌与精确接口目录：呈现目录逐项绑定原21HTTP预设/13协议/4离线引擎；品牌多接口明确确认，建议不等于自动选用或允许切协议。 | AI-CFG-01, AI-CFG-02, AI-CFG-14, FP-06, FP-11 | AI,AP,QA,UI；M04,M05,M13；C26,C27,C29,C31 | catalog, component, design | PRE-01, PRE-13 |
| GD-05 | 必要字段与有来源缺省：每字段分类/类型/标签/条件/来源可查；内置固定或明确site/plan地址由目录带入，资料缺口独立阻断，不让普通用户代填；实际必要项可见，模型不伪填。 | AI-CFG-01, AI-CFG-02, AI-CFG-04, AI-CFG-08, FP-06, FP-11 | AI,AP,QA,UI；M04,M05,M06,M13；C26,C27,C29,C31 | catalog, design, domain, service | CURRENT-SERVICE-MATERIALS, PRE-01, PRE-06, PRE-11, PRE-13 |
| GD-06 | 不适用与内部字段不强填：传统、本机、LLM各只要求适用内容；协议/schema/revision/签名/salt/安全引用内部持有，不把它们变成用户必填。 | AI-CFG-01, AI-CFG-06, AI-CFG-08, AI-CFG-11, FP-06, FP-11 | AI,AP,QA,UI；M04,M05,M13；C26,C27,C29,C31 | catalog, component, design | PRE-01, PRE-13 |
| GD-07 | 遮罩密钥与明确授权：Key/应用密钥用用户语义遮罩，常规草稿仅不透明引用；多字段签名与Google授权不混成通用Key，不伪造OAuth/测试成功。 | AI-CFG-05, AI-CFG-11, AI-CFG-15, FP-06, FP-11, FP-12 | AI,AP,PL,QA,UI；M04,M05,M06,M13,M16；C26,C27,C29,C30,C31 | component, credentials, design, service | CURRENT-SERVICE-MATERIALS, PRE-01, PRE-06, PRE-11, PRE-13, PRE-14, PRE-18 |
| GD-08 | 高级及独立学习按需：适用高级模板/参数/网络/密钥池/运行策略保留；精读/查词独立默认折叠，互不借用/串状态/自动启用。 | AI-CFG-03, AI-CFG-07, AI-CFG-08, AI-CFG-09, AI-CFG-11, AI-CFG-12, AI-CFG-16, FP-06, FP-08, FP-11 | AI,AP,QA,UI；M04,M05,M06,M09,M13；C22,C26,C27,C29,C31 | component, design, domain, service | CURRENT-SERVICE-MATERIALS, PRE-01, PRE-06, PRE-11, PRE-13 |
| GD-09 | 保存选用与版本启停保持：编辑/保存/样句不切服务；选择仍限已添加已保存合法资源，当前版本失败禁选，后续句/范围/启停/重译与迟到历史规则保持。 | AI-CFG-03, AI-CFG-04, AI-CFG-05, AI-CFG-10, AI-CFG-12, AI-CFG-13, AI-CFG-16, FP-04, FP-06, FP-09, FP-12 | AI,AP,PL,QA,UI；M04,M05,M06,M10,M12,M13；C26,C27,C29,C31,C34,C35 | design, domain, native, service, storage | CURRENT-SERVICE-MATERIALS, PRE-01, PRE-06, PRE-08, PRE-10, PRE-11, PRE-13, PRE-14 |
| GD-10 | 本机条件未知仍阻断：四本机引擎按API/传统策略/安装/资源/语言对/运行路线门禁；未知必要条件、缺资源、未实现路线不可用，不以简化表单放行。 | AI-CFG-04, AI-CFG-05, AI-CFG-06, AI-CFG-14, FP-06, FP-11, FP-12 | AI,AP,PL,QA,UI；M04,M05,M06,M12,M13,M15；C26,C27,C29,C31,C32,C33,C34,C35 | design, native, worker | CURRENT-OFFLINE-LICENSE, PRE-01, PRE-08, PRE-10, PRE-11, PRE-13 |
| GD-11 | 自定义必要字段不能隐藏：适用兼容服务的自定义地址、认证、实际模型与服务要求的项目/区域在条件成立时可见且校验；不退回默认协议/地址/模型。 | AI-CFG-01, AI-CFG-02, AI-CFG-04, AI-CFG-08, AI-CFG-11, AI-CFG-14, FP-06, FP-11, FP-12 | AI,AP,QA,UI；M04,M05,M06,M13；C26,C27,C29,C31 | component, design, domain, service | CURRENT-SERVICE-MATERIALS, PRE-01, PRE-06, PRE-11, PRE-13 |
| GD-12 | 可访问样式与证据边界：键盘/焦点/窄窗/长文/主题及200%分别留真实截图和限制；独立候选/源hash，旧计划/批准图/产品not_run不提升。 | FP-04, FP-06, FP-09, FP-11, FP-12, RW-31, RW-32, RW-33, RW-34 | AI,AP,PL,QA,UI；M03,M04,M05,M10,M12,M13,M15；C16,C17,C18,C26,C27,C29,C34,C35,C36 | component, design, game, native, review | CURRENT-GUI-REVIEW, PRE-01, PRE-07, PRE-08, PRE-10, PRE-12, PRE-13, PRE-17 |
| GUI01 | 当前服务连接的代理可达且独立保存：代理仅改当前连接；保存重开仍保留，其他连接和当前绑定/自动开关不变。 | AI-CFG-01, AI-CFG-11, FP-06, FP-11 | AI,AP,QA,UI；M04,M05,M06,M10,M13；C26,C27,C29,C31 | component, design, domain, protocol, storage | PRE-01, PRE-13, PRE-14 |
| GUI02 | 学习用途各自保留温度三态、模板覆盖与预览：grammar/context_word逐用途验证继承/设置/省略温度、模板覆盖与预览身份，互不改写且不外发未授权正文。 | AI-CFG-03, AI-CFG-07, AI-CFG-08, AI-CFG-09, AI-CFG-16, FP-06, FP-08, FP-11 | AI,AP,QA,UI；M04,M05,M06,M09,M13；C22,C26,C27,C28,C29 | component, design, domain, service | CURRENT-SERVICE-MATERIALS, PRE-01, PRE-06, PRE-11, PRE-13 |
| GUI03 | 资源草稿隔离用途策略并只在明确选用后应用：新增/保存未绑定资源不改任何用途；预览区分草稿/已应用，明确选用后仅后续句冻结新版本。 | AI-CFG-03, AI-CFG-04, AI-CFG-10, AI-CFG-13, FP-06, FP-11 | AI,AP,QA,UI；M04,M05,M06,M10,M13；C26,C27,C29,C31 | component, design, domain, service, storage | CURRENT-SERVICE-MATERIALS, PRE-01, PRE-06, PRE-11, PRE-13, PRE-14 |
| GUI04 | 主阅读复制与重译使用现有记录处理流程：复制准确当前原文；独立重译新建记录并保留旧记录/来源，不改服务和自动开关；新句到达不偷换目标。 | AI-CFG-13, FP-04, FP-06, FP-09, RW-17, RW-31, RW-32 | AI,AP,PL,QA,UI；M03,M04,M05,M10,M12,M13；C17,C18,C23,C26,C27,C29,C34,C35 | component, design, domain, game, storage | PRE-01, PRE-07, PRE-08, PRE-10, PRE-12, PRE-13, PRE-14, PRE-17 |
| GUI05 | 失效测试退出运行态且资源任务独立结算：运行中修改令精确任务stale并可重新测试；A/B完成与取消独立，迟到不覆盖新版本或其他资源。 | AI-CFG-04, AI-CFG-05, AI-CFG-12, AI-CFG-13, FP-06, FP-11, FP-12 | AI,AP,QA,UI；M04,M05,M06,M13；C26,C27,C29,C31 | component, design, domain, service | CURRENT-SERVICE-MATERIALS, PRE-01, PRE-06, PRE-11, PRE-13 |
| GUI06 | 凭据组观察与显示名不得洗掉已知认证失败：池观察/显示名变更不改请求凭据身份或原失败fingerprint；已知认证失败继续禁选且保留原因。 | AI-CFG-05, AI-CFG-11, AI-CFG-15, FP-06, FP-11, FP-12 | AI,AP,PL,QA,UI；M04,M05,M06,M13,M16；C26,C27,C29,C30 | component, credentials, design, domain, service | CURRENT-SERVICE-MATERIALS, PRE-01, PRE-06, PRE-11, PRE-13, PRE-14, PRE-18 |
| GUI07 | 传统翻译用途预览只展示适用参数：真实传统请求和预览均不带LLM prompt/model/temperature/history，保留适用语言与运行策略。 | AI-CFG-01, AI-CFG-06, AI-CFG-07, AI-CFG-08, FP-06, FP-11 | AI,AP,QA,UI；M04,M05,M06,M13；C26,C27,C28,C29 | catalog, component, design, protocol, service | CURRENT-SERVICE-MATERIALS, PRE-01, PRE-06, PRE-11, PRE-13 |
| GUI08 | 明确选用大模型后上下文范围可编辑且预览与保存隔离：已选LLM翻译用途可编辑0/正上限与原文/译文/both范围；保存不应用，不串学习；明确重选才冻结策略。 | AI-CFG-03, AI-CFG-04, AI-CFG-09, AI-CFG-13, FP-06, FP-09, FP-11 | AI,AP,QA,UI；M04,M05,M06,M10,M13；C26,C27,C28,C29 | component, design, domain, service, storage | CURRENT-SERVICE-MATERIALS, PRE-01, PRE-06, PRE-11, PRE-13, PRE-14 |

当前设计锚点按对象使用guided候选 `#settings-window`或`#main-window`，JSON逐项保存；下层精确身份仍来自Rust资源/用途/当前句/会话，不从设计DOM推断产品状态。

## 完整绑定旅程附加义务

[绑定旅程机器计划](binding-journey-plan.json)固定BJ-01～18，来源为FP-03/05/09/12、现行GL/RW及已批准目标面板/候选交互，累计于原153、RW、70来源/576观察与901设计义务之外。原CaptureSheet22态完整保留；当前新overlay22态及connecting20态均缺独立参照，另有1项旧入口退场检查，C13～15为partial/source_only；绑定流程仍未完成。

| 用例 | 必须可观察的行为 | 层 |
|---|---|---|
| BJ-01 | 同一目标核对入口与系统确认位置；选择本身不注入 | component, native |
| BJ-02 | 生产错误状态与首次确认动作；准确提示需要系统确认 | domain, component, native |
| BJ-03 | 候选到会话移交单窗口选择；保留原选择filter/owner和核验 | component, native |
| BJ-04 | 无意图时不弹系统选择器；无意图调用为零 | component, native |
| BJ-05 | Rust身份复用切换与过期审查；同目标明确复用且不重复注入 | domain, component, native |
| BJ-06 | 真实operation和Core阶段驱动；阶段只依据真实事实 | domain, component, native |
| BJ-07 | 面板关闭隔离；M12独立Core-active订阅；S1待handoff→S2停旧会话前失败仍保S1时预览最终建立，旧callback零且新候选不被动 | domain, component, native |
| BJ-08 | 待确认多选与显式开始阅读；勾选不发提交或窗口命令 | domain, storage, component, native |
| BJ-09 | 保存选择与临时策略；临时不覆盖或清除既有saved | domain, storage, component, native |
| BJ-10 | 完整样本与高级来源；u64以规范十进制字符串原样传递 | domain, component |
| BJ-11 | schema5原子集合与旧库迁移；001-004原字节与历史FK/trigger保持 | storage |
| BJ-12 | 集合成员消费和双链自动资格；两个当前成员均获合法资格 | domain, storage |
| BJ-13 | 同authority原子选择→HUD可见成功→Main隐藏；before真实Main身份、completed最终OS与loss-free milestone顺序分别证明 | domain, component, native |
| BJ-14 | C1排布和目标核对可达；按批准源核区域和滚动归属 | design, component, native |
| BJ-15 | 真实帧、时间与撤权；新成功帧才更新时间 | domain, component, native |
| BJ-16 | 最终同包四游戏与真实翻译；同源同签名最终包和真实PID/资源 | game, service, native |
| BJ-17 | 失败恢复、数据边界与清理；保留真实状态、草稿和旧成功数据 | domain, storage, component |
| BJ-18 | 顺序和证据防绕过；写前非作者回执且精确源未漂移 | guard |

原P1r2/P2r2、首次P3及P1r3/P2r3/P3r3均已真实成立，原件保留。r3按实际语义RED、五类及续写负例、非作者审核和新目录独占准入完成续写；不能重用旧签字授权另一oracle。已有UI40场景、Native源码／265项受控测试和存储真实producer各自有执行原件，不能跨源码切面拼成最终包通过。R93实际采用后，当前[后继QA计划](../../openspec/changes/register-binding-impact-surface-mappings/iteration-card.md)维护接线和消费者；原产品源保持不变，后续严格全执行与正式包仍未完成。基础设施错误不算语义RED，阶段准入不授产品通过。

观察合同采用：唯一sourceId可见来源行/复选框，不因高级详情重复Hook名而误定位；pending核真实面板/单次请求/无重复提交；候选阶段由当前operation真实事实和同一Rust来源集合的可用复选框/开始/稍后控件证明，Node与Python/保留DOM同义独算。BJC05的有限范围保留，只有同源/同nonce/同run的完整BJS10独算通过才可展示discharged_by，不能全局忽略blocked。

BJ11私有cfg(test)故障保留父integration与子libtest的实际编译/list/exact/二进制/PID/时间/DB链，并消费真实故障arming与native码。BJ13改用before真实Main身份＋completed最终OS/HUD原图＋同authority完整事件commit<show成功<hide成功；不要求定格同步中间瞬间，不改时间或截断watermark。仅BJ13采用该观察形态，BJ14整页Native独立参照和零RGBA阈值完全不变。

运行资料：真实组件需固定Playwright与Rust可编译依赖；存储需真实冻结Store4及原测试源/编译锁闭包、schema4备份副本、独立v5完整合同与故障注入。历史4继续验原81表/734字段及升级/备份/故障，当前5另验83表/748字段、14新增字段/5FK/6trigger/当前成员view与0～4→5/reopen；原v4比较器仍精确拒绝5。实际SQLite/ledger/字段/route原件按同nonce独立复核，不能只改版本预期或借complete-v3执行器宣称完整v4/v5通过。Native需同证书包、可见桌面/系统单窗与录屏能力；四游戏需合法资源/可推进存档及真实服务地址/model。原件维护在fresh `.build/binding-journey-alignment-20261109/`，用例定义不回填通过。

## r3可执行补充合同与责任

P1r3初次独审`attempt-01`的changes_required与全部失败原字节保留；修订计划和P2r3已独审，P3r3准入已实际签发。完整机器义务见[binding-journey-plan.json](binding-journey-plan.json)的`execution_obligations`与`resume_epoch.observation_contracts.storage_parent_child.production_timeline`。本地生产者已有部分实际执行，下面的固定义务不因实现存在而自动通过：Native writer、OS、独立参照、同包四游戏／服务缺项继续blocked或not_run，所有运行另记。当前QA维护只修全局登记、工件消费、真实派发和独算，不改原18BJ／EX9、153／RW、设计义务或原用例预期。

新工程readiness必须真实fresh执行原全BIND并独立复核，前六组本地EX与schema5严格通过才可判评估完成；任何语义、缺本地producer、基础设施、validation及已提供但不合法的证据均失败。允许缺省的只限固定Native／游戏／服务输入，原full仍blocked且保留实际非零raw，产品incomplete。历史九套Store4改以父安全归档的源／锁、真实编译、封存copy、完整list及126项零skip raw核验；共享target合法覆盖不再误报，当前Store5完整父子/时间/DB/故障合同不变。最终运行写入fresh `.build/binding-engineering-integration-20261110/` 或根verify回执，不覆写旧失败。

| 固定义务 | Owner与真实执行路径 | 原件与独立判断 |
|---|---|---|
| EX-LIB-GLOBAL / EX-LIB-SCOPED | UI＋AP/HK；原38组件scene全部保留，新增`BJS-01-library-global-owner`、`BJS-01-scoped-reconnect-owner`；现有Rust `binding_journey::bj_component_transport`＋生产MainWindow/CaptureSheet | 实际SqliteStore/Runtime `SaveGames`保存A/B，返回gameResults与初始snapshot；DOM选A，页头全局入口选运行B应默认既有B。seed后save_game/save_games为0、确认前associate/attach为0、确认后B恰好一次；scoped恢复保留真实意图。Python从实际Rust snapshot/target/review上下文独算ID，不接受自由expectedGameId或单game样例。 |
| EX-LIFE-CORE | AP/HK；现有`cargo test --locked -p app-core --test binding_journey bj_07_domain -- --exact --nocapture`，原断言保留 | 窄authority已实现；分别实际S2资源/target-lease停旧会话前失败、S1仍active，获取fresh current-preview authority；profile/server/full active witness竞态的回调必须0。已有关联切面的受控执行，最终当前源需重验。 |
| EX-LIFE-NATIVE-UNIT | PL；coordinator现文件内固定`platform::session_preview_coordinator::preview_cancellation_tests::bj_07_active_session_preview_reconciliation`及机器合同另四个exact test；实际`cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --bin luna-reader <固定完整名> -- --exact --nocapture` | 真实M12独立订阅Core-active，受控低层barrier/collector ack，不挂UI、不靠直接ensure。旧handoff0、新candidate不变、陈旧witness拒绝；busy/release重复singleflight，healthy/paused不重启，无window/无明确授权不picker。编译artifact/list/exact/源/nonce/PID/原日志和真实事件表由Python独算；不冒充OS。 |
| EX-LEASE-NATIVE-UNIT | PL；WindowPreviewService现文件内固定`bj_07_nonblocking_retirement_admission`、`bj_07_old_retirement_cannot_release_new_candidate` | 真实2active＋1draining/pending原预算、精确ticket/owner/Fence、guard进出与enqueue、实际release ack/admission顺序；guard内不能等待Stop、不能旧ticket释放新candidate。 |
| EX-STORAGE-TIMELINE | Storage实现、QA门禁消费；实际source_selection_sets父integration＋database私有cfg(test)子libtest | 生产open/备份落盘/事务/DDL/native失败/return及回读观察已实现并实际执行；旧AfterDdl时间0失败保留，不能据此把新producer重标尚未实现。Scope创建≠API入口，arming≠native故障，按route与严格顺序核，缺失为null＋not_reached，不用0或父relay。历史消费者修复后仍须fresh严格全验，不能借局部时间链通过宣称schema5完成。 |
| EX-NATIVE-SUCCESS-ROLES | PL/Root真实签名包；原固定名`bj_08_native`、`bj_09_native`（当前未实现），实际app-infra Native writer | 成功beginReading后SourceSheet已卸载且Main已隐藏，committed/temporary/saved采真实HUD＋此前识别Main隐藏＋原DB/journal提交权威。BJ09保存意图/保存后检查另以正常显示Main→重开文本流只读观察，禁止重注入/重提交/改set/重复原效果；不能重开掩盖原窗口效果失败，重开draft为false合法，saved须真实remember=true及saved_snapshot。temporary要真实非空保存基线。 |
| EX-NATIVE-PROGRESS-TRACES | PL/Root真实签名包；固定`bj_06_native`（当前未实现） | 固定waiting-observed/fast-candidates两条清单，按实际当前operation/source事实与OS区间独算选路。waiting实拍等待→候选，fast只要求实际当前候选，历史握手/连接/source顺序由loss-free账本证明，不强造四张中间截图或产品延迟；无首次progress原件不宣称首次轮询，域/真实React迟到与过早阶段义务不减。 |
| EX-LIFE-SIGNED-NATIVE | Root/PL/QA；同源最终签名包BJ07，另同包四游戏/真实服务 | Native-unit不能代OS。保留原五句/款、Never7跨三页、预览/Hook/词卡/翻译与退出恢复、原153/RW/901全部独立义务。 |

P2任务`R3-02-EXEC`/`R3-02-STORAGE-TIME`/`R3-02-NATIVE-OBS`的精确registry、raw与独立消费已在r3门禁中登记；P3已准入，但`R3-04-EXEC`/`R3-04-STORAGE-TIME`的本地实现和`R3-04-NATIVE-WRITER`尚缺的真实OS生产者必须分别验收。缺producer仍blocked/failed，注册不使其通过。LIB01～05、LIFE01～08及LEASE01～04逐项保留，原76定义、Native十四类反例、BJ14整页零RGBA及全部存储义务不变。后继QA准入只授权已审门禁路径，不授权新产品writer或安装包。

## 变体、平台与真实观察

- **目录/协议/服务**：PA-01/GD-04逐25服务核tuple；PA-04等逐13协议核实际路由、认证、签名、语言、参数和终止；GD-05/GD-11逐21 HTTP服务实际请求与恢复。PA-11逐allowed_purposes展开，传统仅translation。GUI02逐grammar/context_word，GUI08逐0/正上限及original/translation/both。模型枚举不等于翻译/学习。
- **本机**：GD-10/OT-11/OT-16逐4引擎。Apple系统能力/许可、Argos合法语言包/route、J北京/金山同位数隔离worker各自证明；目录或路径存在不证明安装、授权或健康。必要条件未知阻断，真实断网/出站观察证明未静默上云。当前J北京/金山强制CrossOver，Apple/Argos按Mac路线独立检查。
- **四游戏**：PA-12/OT-16/GD-12/GUI04按SR/MO/Never7/サクラノ詩展开，来自同一个最终包，各至少5个真实不同字幕状态，Never7至少3个可确认页。逐状态保留稳定state/page身份、字幕hash、游戏/客户端配对及callback归属；事件数不当句数。game_capture和client_capture的文件SHA分别在状态间唯一，复制同字节改名或任意不同图片都不能证明推进。
- **权限**：首次、拒绝、取消、撤销和返回恢复分别取证；缺许可不称live、不影响独立Hook，旧帧保留真实时间和非实时原因。
- **Vault**：遮罩输入、实际保存重启、取消撤销、导出/日志无秘密及typed认证失败。池观察和改名不清掉已知失败；普通资源只存opaque引用，不将真实秘密写入目录、截图或日志。
- **存储**：真实SQLite/Repository验证保存重启、只读/磁盘满、迁移回滚、完整/过滤导出，保留序列和恢复证据；计划/DDL不代实际执行。
- **视觉/可访问**：深浅、窄窗长文、键盘/焦点/Escape、200%分别实拍。200%同时要求整窗无裁切及全部操作可达，DOM仍在或入口可获焦点不够；已知裁切保留待修及限制，不重拍自批或放宽阈值。

OT-16正式包层逐4引擎×4游戏展开16项，独立worker结果与四款聚合结果不能互相顶替。每项固定engine ID/contract revision/backend/runtime、准确包/nonce、合法资源版本/文件hash/许可证据、每状态实际结果和独立质量审阅。SR既有中文版按真实非日语翻译、同语种直通或非日语降级分别记录；未调用引擎不记翻译质量，也不记日语质量。其他三款必须真实日语翻译及独立质量结论。资源未ready、未授权、unknown或route不支持保持blocked，不因其他引擎通过解除。

4项Windows原机资格保留独立环境、同位数worker/DLL ABI、许可资源版本/架构/hash及Windows工件政策，不能复用Mac.app hash或写成Mac通过。未来声明Windows范围必须另工作项审查范围和真实准出。

前置沿[PRE-01～18供给登记](first-product-test-operations.json)。CURRENT-SERVICE-MATERIALS由AI/AP供给真实站点/协议/账户/OAuth材料与预算；CURRENT-OFFLINE-LICENSE由PL/QA供给合法资源/平台；CURRENT-GUI-REVIEW由UI/QA供给准确状态截图及独立审阅。缺材料不能转嫁普通用户填写本应内置资料或冒称就绪。

## 当前设计对齐与执行能力

现有current_design_alignment与全页状态合同继续提供设计来源、真实依赖和页面/状态义务；生产路径决定实际受影响集合，未知共享/native影响保守处理。实现者不能用kind或metadata省略真实覆盖。

当前视觉裁决采用G09：只给同一份设计/真实实现并排报告，按当前实际client尺寸、主题、数据和状态核对；人工明确确认后由程序放行。缺图、不可比、待审、拒绝及相关变化均阻断，旧设计源批准和旧4态参照不自动授新产品通过。旧像素/有效render失败保留历史，不通过重拍或改阈值消除。

图像与元数据只证明来源/可比性的一部分，不代替用户语义、交互、原生和真实游戏测试。原802/901/944状态及所有业务观察保留，报告形式简化不删除测试义务。

## 分层证据、准出入口与维护

catalog/domain/component/storage/protocol/pipeline/credentials使用source_build；真实服务/worker/原生/游戏/独立实拍评审使用final_package；design独立，不代产品。每层要求真实动作/结果、精确对象/版本/来源、完整证据kind及非空文件/hash。代码能力表限定case、layer、execution ID、scope、command和schema，JSON不能安装adapter；当前新增业务生产表为空。

原发布schema2以 `current_product_coverage:{path,sha256}`侧录schema1追加当前registry hash、同一artifact hash和完整576观察回执；视觉由G09消费唯一对照报告、manifest与真实确认来源；旧 `current_design_alignment` 原件保持历史身份。原153执行集合不接收新ID。回执和原日志匹配当前源码/包/nonce/精确command、正断言/零skip、时区起止与观察时间；详见[发布证据](release-execution-evidence.md)。hash和结构只证明身份/内部一致性，不能独立证明任意文字声称的系统事件。

```sh
python3 scripts/quality/current_product_coverage.py --check --json
python3 scripts/quality/current_product_coverage.py --assess --evidence .build/<real-run>/release.json --artifact dist/<actual-artifact> --json
python3 scripts/quality/current_design_alignment.py --check --json
python3 scripts/quality/current_design_alignment.py --assess --evidence .build/<real-run>/release.json --artifact dist/<actual-artifact> --json
```

静态check只验合同/来源；assess只审当前真实证据，缺能力/参照/确认/同包结果须blocked/exit1，不执行产品。统一入口无条件检查当前业务与设计合同；开发产品按当前目标阻断，release同时强制原153、RW、真实存储、新576、完整页面状态、设计批准及其他准出。单项实际执行只能解除自身缺口，不授予完整设计确认或替代其他集合。

QA维护合同/覆盖/准出，各AI/AP/UI/PL/HK/JA按逐项Owner协作。规则由 `current_product_coverage.py/test_current_product_coverage.py`及 `current_design_alignment.py/test_current_design_alignment.py`维护；新增需求/层/平台先审合同再扩能力。临时合成receipt/adapter仅用于guard正反例，不注册生产或写产品结果。每次运行另建私有run，失败保留，原计划不回填；默认不提交游戏原文/截图、私人路径或秘密。

旧正文原字节保存在[文档归档](../archive/documentation-r75/docs/development/current-product-test-coverage.md)，只供追溯，不作为现行执行入口。
