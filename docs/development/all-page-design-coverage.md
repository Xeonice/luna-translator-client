# 当前完整页面与状态闭包

目标：`guided-service-reading-20261030`；完整组合：`complete-guided-history-20261031`。本表来自当前 TypeScript AST 实际目录及代码权威状态义务。新页面、弹层、共享组件和新状态必须补映射。

完整设计源由[完整批准](design-approval-complete-20261031.json)绑定原57源与R79历史4源；C35以[已采用五项组件补充](design-approval-preview-20261104.json)为新目标，比较画布不替代完整主窗。CSS wallpaper另作依赖。原4项有限真实React检查和General14状态的React/同包Native执行分别限定能力，其他状态仍缺；当前执行选择必须从已注册adapter.states逐层求覆盖。设计确认、独立参照、真实组件与同包原生分别留证；原29状态、原PNG/token/CSV和全部义务保留，不能由登记或有限能力宣布产品通过。

每条状态分别记录入口query、设计操作recipe、target、固定fixture源身份、需求/组件/Owner和实际层。`existing_design_state`已登记可回放recipe与具体状态断言；是否实际执行以独立运行回执为准，仍不代表参照或产品通过；`unmapped_design_state` / `needs_mapping`只表示尚未完成可核对映射，不能推断设计不存在；只有逐项理由和固定源码anchor支持的状态才标`missing_design_state`。纯展示组件按实际props标not_applicable；明暗/窄窗/200%/长文另列视觉变体与未执行的参照/数据/几何测量。

## 实际页面矩阵

| 实际surface | 生命周期 | 页面/弹层/组件 | 适用义务 | 已映射 | 待映射 | 已核实设计缺项 | 实现差距 |
|---|---|---|---:|---:|---:|---:|---|
| `main:library` | active_production_surface | 游戏库 | 13 | 4 | 9 | 0 | 生产/设计游戏记录、封面及窗口关联需当前参照复核。 |
| `main:session` | active_production_surface | 当前会话与三个独立数据集 | 26 | 15 | 11 | 0 | 需当前逐状态参照与实际执行 |
| `main:history` | active_production_surface | 历史记录与人工修正 | 20 | 17 | 3 | 0 | 历史目标已纳入完整批准组合；正式HistoryScreen仍须迁移会话目录/记录独立查询与R79轻量操作，详情加载/错误及revision冲突仍待独立映射。 |
| `main:manual` | active_production_surface | 手动翻译 | 15 | 5 | 10 | 0 | 生产源语言选择与设计固定日中提示不同；不得默认为一致。 |
| `main:diagnostics` | active_production_surface | 主窗诊断与恢复 | 15 | 4 | 11 | 0 | 需当前逐状态参照与实际执行 |
| `window:main` | active_production_surface | 主窗壳与启动恢复 | 15 | 2 | 13 | 0 | 需当前逐状态参照与实际执行 |
| `window:hud` | active_production_surface | 阅读浮窗 | 26 | 15 | 11 | 0 | 需当前逐状态参照与实际执行 |
| `window:learning` | active_production_surface | 独立学习与内置词典 | 30 | 7 | 23 | 0 | StudyScreen具有队列/流式与查询错误，旧learning脚本未实现全部对应渲染。 |
| `window:settings` | active_production_surface | 设置窗口壳、搜索及返回 | 12 | 4 | 8 | 0 | 需当前逐状态参照与实际执行 |
| `surface:word-card` | active_production_surface | 原生临时词卡独立surface | 18 | 0 | 18 | 0 | HTML的#word-popover不是生产?surface=word-card原生surface等价参照；需独立明确布局/边界及实际内容撑窗状态。 |
| `settings:general` | active_production_surface | 通用 | 14 | 14 | 0 | 0 | 独立General源/参照/真实组件/同包Native分别验收，受控视图不证明OS、存储或游戏。 |
| `settings:capture` | active_production_surface | 输入与HOOK | 15 | 2 | 13 | 0 | 设计含记住来源、去重/合并字段；生产该页目前为摘要和返回入口。 |
| `settings:ocr` | active_production_surface | OCR范围说明 | 6 | 2 | 4 | 0 | 首版排除OCR；只验证说明、无执行入口，不新增加载或运行能力。 |
| `settings:processing` | active_production_surface | 文本处理 | 13 | 2 | 11 | 0 | 设计可编辑去重/合并，生产当前为说明；差距未关闭。 |
| `settings:speech` | active_production_surface | 语音范围说明 | 6 | 2 | 4 | 0 | 首版排除语音；只验证说明且无执行入口。 |
| `settings:display` | active_production_surface | 显示与快捷键 | 21 | 8 | 13 | 0 | 原生材质/系统减少透明度/真实快捷键不由网页模拟证明。 |
| `settings:network` | active_production_surface | 网络范围与连接入口 | 7 | 2 | 5 | 0 | 仅连接网络策略及后续范围说明；实际连接在翻译服务维护。 |
| `settings:data` | active_production_surface | 配置与数据 | 20 | 6 | 14 | 0 | 设计含导入预检/安全导出/恢复草稿；生产此分类仅配置摘要与资源检查，不能宣称对齐。 |
| `settings:diagnostics` | active_production_surface | 设置诊断 | 13 | 2 | 11 | 0 | 设置中的配置诊断与生产复用主窗DiagnosticsScreen的布局/字段不同。 |
| `settings:translation` | active_production_surface | 引导式翻译服务配置 | 31 | 2 | 29 | 0 | 生产当前仍为用途/连接/模型/参数/运行/术语分页，不是guided服务编辑目录；布局与公共行为差距保留。 |
| `overlay:CaptureSheet:e1434a9e7d` | retired_surface_with_retained_obligations | 运行目标与正文来源 | 23 | 2 | 21 | 0 | 原CaptureSheet入口身份已退场；22条历史状态与source descriptor保留，新弹层及connecting真实状态需独立精确设计映射、参照与执行，均不据登记通过。 |
| `overlay:LibraryScreen:f3c016c929` | active_production_surface | 添加／编辑／批量核对游戏 | 18 | 2 | 16 | 0 | 生产弹层与当前设计的对象/版本/引用/布局需要独立参照；未可重放状态保持blocked。 |
| `overlay:LibraryScreen:828e15dd12` | active_production_surface | 移出游戏库确认 | 13 | 2 | 11 | 0 | 生产弹层与当前设计的对象/版本/引用/布局需要独立参照；未可重放状态保持blocked。 |
| `overlay:HudWindow:6f81579d6a` | active_production_surface | HUD更多阅读菜单 | 15 | 2 | 13 | 0 | 生产弹层与当前设计的对象/版本/引用/布局需要独立参照；未可重放状态保持blocked。 |
| `overlay:ModelConfigScreen:92d4a07590` | retired_surface_with_retained_obligations | 配置应用确认 | 15 | 0 | 15 | 0 | MC-12保存、显式选用、版本和在途归属由guided编辑页及统一选择器承担；不能由保存推定已应用。 原状态ID、适用性、两实际层与独立参照缺口不变；新增旧入口缺席负例不代替功能验收。 |
| `overlay:ModelConfigScreen:d6953cce06` | retired_surface_with_retained_obligations | 模型复制确认 | 16 | 0 | 16 | 0 | MC-08完整服务复制仍需交付；当前生产缺复制入口，原共享连接/名称/版本/失败/取消义务继续blocked。 原状态ID、适用性、两实际层与独立参照缺口不变；新增旧入口缺席负例不代替功能验收。 |
| `overlay:ConfigurationLifecycleActions:8626d9f075` | active_production_surface | 删除连接或模型确认 | 15 | 0 | 15 | 0 | 生产弹层与当前设计的对象/版本/引用/布局需要独立参照；未可重放状态保持blocked。 |
| `overlay:ConfigurationLifecycleActions:9fb87066f2` | active_production_surface | 连接复制确认 | 13 | 0 | 13 | 0 | 生产弹层与当前设计的对象/版本/引用/布局需要独立参照；未可重放状态保持blocked。 |
| `overlay:CredentialRevocationPanel:7f58db8661` | active_production_surface | 系统凭据撤销确认 | 14 | 0 | 14 | 0 | 生产弹层与当前设计的对象/版本/引用/布局需要独立参照；未可重放状态保持blocked。 |
| `overlay:RuntimeConfigurationPanel:3cfa833a01` | active_production_surface | 恢复已应用连接确认 | 13 | 0 | 13 | 0 | 生产弹层与当前设计的对象/版本/引用/布局需要独立参照；未可重放状态保持blocked。 |
| `overlay:RuntimeConfigurationPanel:4812c5b249` | active_production_surface | 恢复已应用用途确认 | 13 | 0 | 13 | 0 | 生产弹层与当前设计的对象/版本/引用/布局需要独立参照；未可重放状态保持blocked。 |
| `overlay:SessionRecordPanel:208683c199` | active_production_surface | 会话导出确认 | 17 | 0 | 17 | 0 | 生产弹层与当前设计的对象/版本/引用/布局需要独立参照；未可重放状态保持blocked。 |
| `overlay:PreviewDetails:c72c668099` | active_production_surface | 预览详情与只读诊断 | 8 | 5 | 3 | 0 | 详情为真实独立Sheet；复制在途/失败和长事实的同态设计映射仍待补，独立参考与正式包未验收。 |
| `shared:Button` | active_production_surface | 共享组件 Button | 14 | 2 | 12 | 0 | 共享HTML材质示例只作设计来源；真实组件与每个使用容器另验。 |
| `shared:TextField` | active_production_surface | 共享组件 TextField | 12 | 2 | 10 | 0 | 共享HTML材质示例只作设计来源；真实组件与每个使用容器另验。 |
| `shared:Switch` | active_production_surface | 共享组件 Switch | 10 | 2 | 8 | 0 | 共享HTML材质示例只作设计来源；真实组件与每个使用容器另验。 |
| `shared:Select` | active_production_surface | 共享组件 Select | 19 | 7 | 12 | 0 | 共享HTML材质示例只作设计来源；真实组件与每个使用容器另验。 |
| `shared:Sheet` | active_production_surface | 共享组件 Sheet | 11 | 0 | 11 | 0 | 共享HTML材质示例只作设计来源；真实组件与每个使用容器另验。 |
| `shared:ActionMenu` | active_production_surface | 共享组件 ActionMenu | 11 | 2 | 9 | 0 | 共享HTML材质示例只作设计来源；真实组件与每个使用容器另验。 |
| `shared:AnchoredPopover` | active_production_surface | 共享组件 AnchoredPopover | 9 | 0 | 9 | 0 | 共享HTML材质示例只作设计来源；真实组件与每个使用容器另验。 |
| `shared:PageHeader` | active_production_surface | 共享组件 PageHeader | 6 | 2 | 4 | 0 | 共享HTML材质示例只作设计来源；真实组件与每个使用容器另验。 |
| `shared:EmptyState` | active_production_surface | 共享组件 EmptyState | 6 | 0 | 6 | 0 | 共享HTML材质示例只作设计来源；真实组件与每个使用容器另验。 |
| `shared:Group` | active_production_surface | 共享组件 Group | 6 | 2 | 4 | 0 | 共享HTML材质示例只作设计来源；真实组件与每个使用容器另验。 |
| `shared:FormRow` | active_production_surface | 共享组件 FormRow | 7 | 2 | 5 | 0 | 共享HTML材质示例只作设计来源；真实组件与每个使用容器另验。 |
| `shared:Notice` | active_production_surface | 共享组件 Notice | 13 | 0 | 13 | 0 | 共享HTML材质示例只作设计来源；真实组件与每个使用容器另验。 |
| `shared:StatusLabel` | active_production_surface | 共享组件 StatusLabel | 7 | 2 | 5 | 0 | 共享HTML材质示例只作设计来源；真实组件与每个使用容器另验。 |
| `shared:WindowFrame` | active_production_surface | 共享组件 WindowFrame | 7 | 2 | 5 | 0 | 共享HTML材质示例只作设计来源；真实组件与每个使用容器另验。 |
| `shared:Icon` | active_production_surface | 共享组件 Icon | 6 | 2 | 4 | 0 | 共享HTML材质示例只作设计来源；真实组件与每个使用容器另验。 |
| `shared:WindowLoading` | active_production_surface | 共享组件 WindowLoading | 10 | 0 | 10 | 0 | 共享HTML材质示例只作设计来源；真实组件与每个使用容器另验。 |
| `subpage:ModelConfigScreen:tab:purposes` | retired_production_surface | 生产配置子页 · 用途 | 1 | 1 | 0 | 0 | 旧生产页签需要删除/合并；只验证旧导航消失和新section映射，不要求重新设计旧页签。 |
| `subpage:ModelConfigScreen:tab:connections` | retired_production_surface | 生产配置子页 · 连接 | 1 | 1 | 0 | 0 | 旧生产页签需要删除/合并；只验证旧导航消失和新section映射，不要求重新设计旧页签。 |
| `subpage:ModelConfigScreen:tab:models` | retired_production_surface | 生产配置子页 · 模型 | 1 | 1 | 0 | 0 | 旧生产页签需要删除/合并；只验证旧导航消失和新section映射，不要求重新设计旧页签。 |
| `subpage:ModelConfigScreen:tab:parameters` | retired_production_surface | 生产配置子页 · 模板与参数 | 1 | 1 | 0 | 0 | 旧生产页签需要删除/合并；只验证旧导航消失和新section映射，不要求重新设计旧页签。 |
| `subpage:ModelConfigScreen:tab:runtime` | retired_production_surface | 生产配置子页 · 测试与运行 | 1 | 1 | 0 | 0 | 旧生产页签需要删除/合并；只验证旧导航消失和新section映射，不要求重新设计旧页签。 |
| `subpage:ModelConfigScreen:tab:terms` | retired_production_surface | 生产配置子页 · 术语表 | 1 | 1 | 0 | 0 | 旧生产页签需要删除/合并；只验证旧导航消失和新section映射，不要求重新设计旧页签。 |
| `subpage:StudyScreen:tab:grammar` | active_production_surface | 学习子页 · grammar | 14 | 2 | 12 | 0 | 需当前逐状态参照与实际执行 |
| `subpage:StudyScreen:tab:dictionary` | active_production_surface | 学习子页 · dictionary | 14 | 2 | 12 | 0 | 需当前逐状态参照与实际执行 |
| `subpage:ReadingPanel:dataset:body` | retired_production_surface | 会话数据子页 · body | 1 | 1 | 0 | 0 | 旧生产页签需要删除/合并；只验证旧导航消失和新section映射，不要求重新设计旧页签。 |
| `subpage:ReadingPanel:dataset:extraction` | retired_production_surface | 会话数据子页 · extraction | 1 | 1 | 0 | 0 | 旧生产页签需要删除/合并；只验证旧导航消失和新section映射，不要求重新设计旧页签。 |
| `subpage:ReadingPanel:dataset:hook_events` | retired_production_surface | 会话数据子页 · hook_events | 1 | 1 | 0 | 0 | 旧生产页签需要删除/合并；只验证旧导航消失和新section映射，不要求重新设计旧页签。 |
| `subpage:CaptureSheet:list:windows` | active_production_surface | 目标来源子页 · windows | 14 | 2 | 12 | 0 | 需当前逐状态参照与实际执行 |
| `subpage:CaptureSheet:list:processes` | active_production_surface | 目标来源子页 · processes | 14 | 2 | 12 | 0 | 需当前逐状态参照与实际执行 |
| `subpage:CaptureSheet:step:select` | active_production_surface | 接入步骤 · select | 14 | 2 | 12 | 0 | 来源选择/等待候选需要当前对应设计数据与真实接入执行，不能用窗口选择成功替代收到正文。 |
| `subpage:CaptureSheet:step:streams` | active_production_surface | 接入步骤 · streams | 14 | 0 | 14 | 0 | 来源选择/等待候选需要当前对应设计数据与真实接入执行，不能用窗口选择成功替代收到正文。 |
| `overlay:ModelConfigScreen:5fb902a57e` | retired_surface_with_retained_obligations | 添加现有协议服务 | 8 | 0 | 8 | 0 | 品牌/明确接口目录替代三协议添加入口；原七项适用义务继续由目录及编辑宿主承担。 原状态ID、适用性、两实际层与独立参照缺口不变；新增旧入口缺席负例不代替功能验收。 |
| `overlay:ServiceCatalogPicker:8a4aeed99b` | active_production_surface | 品牌与明确接口目录 | 13 | 4 | 9 | 0 | guided-config.js175-178已有品牌/接口目录；生产busy、搜索与逐态交互需要独立同态映射、参照和真实执行。 |
| `overlay:ServiceSelector:ddfdd2de73` | active_production_surface | 统一已保存服务选择器 | 19 | 2 | 17 | 0 | service-picker.js114-140准确支持翻译选择器；生产精读/查词用途、异步失败与逐态行为不能继承翻译静态正常态。 |
| `subpage:ModelConfigScreen:catalogPurpose:translation` | active_production_surface | 添加服务用途 · translation | 8 | 2 | 6 | 0 | guided-config.js178明确列出三用途；仅正常/浅色目录选择有准确recipe，其余逐态义务保持待映射。 |
| `subpage:ModelConfigScreen:catalogPurpose:grammar` | active_production_surface | 添加服务用途 · grammar | 8 | 2 | 6 | 0 | guided-config.js178明确列出三用途；仅正常/浅色目录选择有准确recipe，其余逐态义务保持待映射。 |
| `subpage:ModelConfigScreen:catalogPurpose:context_word` | active_production_surface | 添加服务用途 · context_word | 8 | 2 | 6 | 0 | guided-config.js178明确列出三用途；仅正常/浅色目录选择有准确recipe，其余逐态义务保持待映射。 |
| `overlay:ModelConfigScreen:3e3e52b7fc` | active_production_surface | 服务编辑未保存确认 | 9 | 0 | 9 | 0 | settings.js246的批准源含“保存并切换”；当前生产只有继续编辑/放弃，按钮与行为不等价，全部适用状态needs_mapping，不能标已有精确设计。 |
| `overlay:SettingsWindow:ca82dd59c6` | active_production_surface | 离开服务页未保存确认 | 9 | 0 | 9 | 0 | settings.js246的批准源含“保存并切换”；当前生产只有继续编辑/放弃，按钮与行为不等价，全部适用状态needs_mapping，不能标已有精确设计。 |
| `overlay:ModelConfigScreen:6da213484c` | active_production_surface | Google授权撤销确认 | 15 | 0 | 15 | 0 | 当前批准整稿无Google撤销确认Sheet的精确状态；PRD AI-CFG-11要求新增授权/秘密输入界面补当前整稿并实机验证。原系统凭据撤销确认不能替代Google本地撤销/远端确认状态。 |
| `overlay:CaptureSheet:afa78bce55` | active_production_surface | 运行目标、接入进度与正文来源（当前AST） | 22 | 0 | 22 | 0 | 生产Sheet由select/connecting/streams和progressFailed分支组成，旧弹层正常态的recipe不能冒当前逐态参照；全部新状态needs_mapping，实际层not_run，产品blocked。 |
| `subpage:CaptureSheet:step:connecting` | active_production_surface | 自动接入阶段与显式失败重试 | 20 | 0 | 20 | 0 | 真实step=connecting、八个AssociationStage与关闭保留操作/显式重试来自CaptureSheet；批准源无该完整同态回放，不能用静态选择页或JSON登记替代设计/真实executor。 |

设计recipe的evaluate_design_api明确按invoke_function_body执行脚本体，不能返回未调用的函数；异步操作需等待。每个已有状态另有可见控件/属性/状态断言。入口query在代码中逐surface/状态合并一次，浅色不会二次navigate丢失historyState；历史窄窗740×1200，200%使用实际CSS zoom=2。recipe JSON合同通过只证明登记，真实回放、几何/裁切和产品执行另留证。

旧ModelConfig六实体页签与Reading三数据页签标为retired_production_surface：只保留旧导航不得存在的负例与当前guided服务编辑/用途高级设置、同页折叠数据集映射。它们可以从实际AST消失，不要求重新设计或复原旧页面；terms具体编辑位置尚待映射，不能用旧术语表tab替代。

旧ModelConfig添加/应用/复制三个Sheet采用独立的retired_surface_with_retained_obligations策略：[冻结迁移身份](model-config-surface-lineage-20261109.json)绑定原856条记录、780项all-page适用义务与完整809状态定义。原三个Sheet的39条记录（36项适用）、全部ID/适用性/required layers/blocked状态及原设计descriptor逐项保留，另加3项AST缺席负例；六实体tab的旧快捷退役规则不适用。MC-12保存、显式选用、版本和在途归属继续由当前编辑页/选择器承担；MC-08完整复制仍无生产入口，不能由复制旧Sheet退役抹掉。后续实际入口按精确身份差集累计追加135项（包含原92项与本轮CaptureSheet增量）；新同步目录/确认的16条不适用记录仍保留并逐条说明宿主职责。全局944适用状态不表示产品通过。

CaptureSheet保留旧e143入口的22条状态与完整source descriptor，追加独立AST缺席；实际afa78当前Sheet及connecting子页不沿用旧recipe授予映射，全量needs_mapping/blocked，两层保持not_run。

品牌/明确接口目录与三个用途入口仅采用guided-config.js175-178的准确recipe；统一选择器仅对service-picker.js114-140的翻译正常/浅色态建立映射。未保存确认与settings.js246“保存并切换”的按钮/行为不同，Google撤销没有当前批准的精确Sheet，均保持needs_mapping；新增授权界面仍须按PRD AI-CFG-11补整稿并实机验证。

## 继承与实际来源

`acceptance-v4/catalogue.json`仅保留历史血缘；旧参照不能覆盖当前完整组合。默认入口仍为guided；`main:history`唯一使用R79历史入口与hx DOM。历史20项中17项已有recipe，独立详情加载/错误及revision冲突3项保持待映射，不以列表加载或保存失败冒充。原20261019的16源中app/data/game-library/index/review-gaps/settings已有变化，页面专属CSS未变不证明整个依赖闭包未变。

当前`scenario=reading`默认只开game/HUD，主会话必须先打开主窗并进入session；`scenario=hud`不是独立分支，抽象`reading settings hud`不是可执行URL。设置子页通过实际分类按钮进入。独立`surface=word-card`不经过WindowRoot四窗口，需要独立参照和同包原生内容撑窗验证。

## 漏项与影响拒绝规则

AST读取MainPage/SettingsPage union与真实导航双向比对、WindowRoot实际分支、bootstrap临时surface、Sheet/ActionMenu实例以及共享UI导出。新增或删除入口不匹配时拒绝。每页闭包递归解析本地TS/TSX barrel imports和CSS依赖；共享Select/Controls/token影响所有实际使用页；无法定位的产品或native影响保守要求全部状态。

检查逐状态对比代码权威义务及source/recipe/target/reference/execution身份；删行、漏状态、旧参考、隐藏target元数据、伪approved、跳过真实产品层均拒绝。合同检查通过只证明完整登记与诚实阻断，不能代替截图、真实交互或发布验收。

```sh
python3 scripts/quality/all_page_design_coverage.py --check --json
python3 scripts/quality/all_page_design_coverage.py --assess --json
node scripts/quality/page-inventory.mjs --root .
python3 -m unittest discover -s scripts/quality -p test_all_page_design_coverage.py -v
```
