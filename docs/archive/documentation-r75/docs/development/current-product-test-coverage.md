# 当前guided目标：逐来源测试与附加准出

修订：2026-10-31。权威目标为最新guided候选的当前阅读优先、历史/提取/Hook后置折叠及必要配置；完整候选仍 `pending_review`。本文件与[current-product-test-coverage.json](current-product-test-coverage.json)同步，原128定义/153组合/97来源和111行CSV保持不变；本登记不追认任何历史产品通过。

新增70来源：PA12+OT16+SS12+FL10+GD12+GUI8；原62记录全文/需求/层/状态保留，GUI8直接核对现有设计runner的八个literal regression调用。按适用层与环境展开576个追加观察身份 `case@layer@variant`。25 services=21 HTTP presets+4 offline engines，HTTP协议13种。不是全笛卡尔积：协议签名/终止用例逐协议，目录逐服务，本机资源逐引擎，当前J北京/金山按CrossOver展开；Windows原机四项独立保留，传统预览GUI07限定实际Azure场景；日常普通凭据不要求所有provider购买两把Key。

每项机器记录包含当前设计锚点、真实需求、Owner/M-C、前置、原断言、操作和可观察期望。下表是人工索引，精确原断言/来源/映射与变体以JSON和代码审查策略为准。所有产品计划状态 `not_run`；真实外部条件不足的运行另记 `blocked`。

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

## 变体与真实观察

- 目录：PA-01/GD-04覆盖25服务精确tuple；协议PA-04等覆盖13协议的实际路由/认证/签名/语言/参数/终止；GD-05/GD-11逐21 HTTP服务实际请求与失败恢复；PA-11按每服务allowed_purposes分别展开translation/grammar/context_word，传统仅translation。GUI02分别展开两学习用途，GUI08展开上下文0/正上限与original/translation/both。不得把模型枚举成功当翻译/学习通过。
- 本机：GD-10/OT-11/OT-16等逐4引擎；Apple能力/系统许可、Argos合法语言包和运行路线、J北京/金山同位数隔离worker分别记录。安装目录不证明资源/许可/健康，必要条件未知阻断；实际断网与出站观察证明不静默上云。OT-08与OT-16中的J北京/金山当前仅强制CrossOver；Apple/Argos按Mac路线独立核验。原Windows四项保留为separate_platform_qualifications，明确环境/引擎ABI/资源和独立Windows工件，不进入当前Mac单包准出，不写passed。
- 四游戏：PA-12/OT-16/GD-12及GUI04分别展开SR/MO/Never7/サクラノ詩；同一最终包，各至少5个不同字幕状态，Never7至少3个可确认页。每状态有稳定身份、字幕hash、游戏/客户端配对与callback来源；事件数量不冒句数。SR为非日语降级，其他三款留日文学习证据。
- 权限：首次、拒绝、取消、撤销、返回恢复逐变体；缺许可不标live、不影响独立Hook；旧帧保留时间和非实时原因。
- Vault：遮罩输入、真实凭据保存重启、取消撤销、导出/日志无秘密、typed认证失败；凭据组观察和改名不能洗掉已知失败。普通资源仅opaque引用，真正秘密不写本目录/截图/日志。
- 存储：保存重启、只读/磁盘满、迁移回滚、完整/过滤导出各有真实SQLite或Repository及序列/恢复证据；接口计划/DDL检查不代实际执行。
- 可访问与视觉：深/浅、窄窗长文、键盘/焦点/Escape、200%实拍；200%须同时无裁切且全部操作可达，不能只检查节点仍在。当前已知整桌面Zoom裁切限制保留为待修与评审项，不自动重拍批准图或放宽阈值。

PRE-01～18沿既有[供给登记](first-product-test-operations.json)。新增CURRENT-SERVICE-MATERIALS由AI/AP补齐各服务真实站点/协议/账户/OAuth材料与许可预算；CURRENT-OFFLINE-LICENSE由PL/QA取得合法资源和对应平台；CURRENT-GUI-REVIEW由UI/QA取得准确候选/状态/200%截图及独立审阅。准备未完成不要求普通用户代填本应内置的资料，不冒称就绪。

## 分层证据与执行边界

`catalog/domain/component/storage/protocol/pipeline/credentials`使用source_build证据；实际服务/worker/原生/游戏/独立实拍评审使用final_package；design独立，不代产品。每层定义自身必需证据kind，要求真实动作与结果、精确对象/版本/来源以及非空文件/hash。机器注册不能自授adapter；代码能力表同时限定case、layer、具体execution ID、scope、command及证据schema。当前新范围生产表为空，已有设计命令仅作追溯引用。

原发布schema2新增 `current_product_coverage:{path,sha256}`，侧录schema1包含当前registry hash、同一最终artifact hash和完整追加execution结果；每行引用真实回执。回执/原日志匹配当前源码/包/nonce/精确command、正断言/零skip、带时区起止和观察时间。更多字段见[发布证据](release-execution-evidence.md)。原153原样独立验证，不能把新ID塞旧execution_results。未来真实adapter必须补实际执行调度与原始观察检查，不能只登记命令或手写passed。

```sh
python3 scripts/quality/current_product_coverage.py --check --json
python3 scripts/quality/current_product_coverage.py --assess --evidence .build/<real-run>/release.json --artifact dist/<actual-artifact> --json
```

静态检查passed只说明闭包与来源有效；准出缺证据/能力为blocked/exit1；工具返回 `not_product_execution:true`。`verify.py`无条件执行current-product-coverage-contract，release无条件current-product-release-coverage。旧153、RW真实层、存储、设计批准及其他发布门禁继续独立强制。当前没有执行新产品测试，不存在新的passed结果；全部历史回执与失败保留。

维护Owner为QA，领域/协议/服务AI/AP、UI及原生PL按逐项映射协作。新增需求或层/平台先审查合同再扩代码策略，不允许JSON缩减必需集合。守卫源码`current_product_coverage.py`与`test_current_product_coverage.py`；临时合成receipt/adapter仅用于守卫正反例，绝不注册生产能力或写入产品结果。执行另建私有run记录，默认不提交游戏原文、截图、路径或秘密。

当前guided完整候选pending_review是独立准出阻断；单项真实执行可解除自身执行缺口，不能授权设计确认。步骤与观察文字是受审计划；机器自动保证原来源断言、层、映射和库存身份，不声称任意新步骤文字已被语义证明。

当前准出scope固定为 `macos-arm64-crossover`：576个必需观察与4项非当前Windows资格分别计数，不能合称全平台通过。四项只允许OT-08/OT-16×J北京/金山，代码逐项核其Windows环境、同位数worker/DLL ABI、许可资源版本/架构/内容hash、独立工件政策和not_run。JSON不能把其他必需观察移出scope。未来声明Windows范围须另工作项审查范围和真实工件，不得用Mac.app hash代替Windows资格。

逐游戏的五状态还分别要求 `game_capture` 和 `client_capture` 的真实文件SHA在状态间唯一；复用同图或改名复制同字节不得冒充不同字幕状态。该核对只证明文件内容身份不同，真实adapter仍须验证字幕、页及callback归属，不能由任意不同图片或声明hash伪造游戏推进。

OT-16当前正式包层按4引擎×4游戏显式展开16项，不能由独立worker检查和四款聚合game结果互相顶替。每项冻结engine ID、contract revision、backend、Mac/CrossOver runtime，并要求精确该引擎/游戏/包/nonce的逐状态实际译文、合法资源版本/文件hash/许可证据及独立质量审阅；SR非日语分支仍如实记录。四款至少各五不同游戏/客户端画面，Never7至少三页。原Windows4资格仍在当前scope之外独立not_run。

OT-16中SR既有中文版只按真实同语种原文、直通、非日语降级或实际非日语翻译分别记录；直通/未调用引擎的证据只能通过非日语分支检查，不得记日语译文质量。其他三款日文游戏必须有该引擎真实调用、原译文及独立日语质量结论。每引擎/游戏的资源状态必须ready、已授权且route支持；缺授权/未知/不支持仍blocked，不因直通、显示中文或其他引擎通过而绕过。
