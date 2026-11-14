# 客户端整页实施设计

## 1. 依据、边界与当前状态

本变更执行用户已审核的R90整页计划，覆盖CP00～11、ACD001～011及FP01～12。目标为guided＋R79完整61源、C35五项补充及独立批准的C1容器：Main／历史1040×720，设置A1000×720；760×560时预览顺延到正文下方。配色、材质、功能及游戏尺寸不因容器适配改变。

产品仍incomplete，next_stage_allowed=false。“已实现”仅指源码及限定测试，不代表已安装包、真实服务、原生行为或准出。原R90基线、批准源/图、License、冻结义务及红日志保持身份；禁止刷新hash、改预期或降阈值消除缺口。现行规范覆盖维护，历史实施原件另存。

Rust Core持有业务、配置和授权权威，app-infra实现SQL／Vault／HTTP／文件／子进程。Tauri只鉴别窗口角色、接收typed命令、调度及装配；React消费生成DTO与ApplicationPort，不直连HTTP／SQL／FFI。hooks-core、collector及语言worker保持独立。原共享Runtime继续管理真实任务、重试、取消、缓存和终态，不造第二条业务队列。

## 2. 注册、字段与配置旅程

M06不可变Protocol／CompatibilityProfile／ProviderPreset精确注册13协议、21 HTTP预设、4离线引擎，不含腾讯TMT。builtin／user分开；未知、重复、版本、用途或认证冲突在Vault／网络前拒绝。目录支持、实际handler装配、资源资格与某次测试通过是不同事实。

用户从用途→品牌→明确接口进入，只填必要字段；高级、网络、学习及诊断按需展开。普通路径不沿内部连接→模型→用途实体顺序。条件、默认、认证解释和必要性由同一Rust目录投影；UI不读取设计JSON，不抄品牌能力表。秘密只经专门mutation写Vault，普通values、快照和导出不含秘密。

纯投影检查未知字段、类型、choice、有限数、长度、NUL与依赖；未知条件不造默认。defaultBy跟随明确选择，不取首项；隐藏旧值不得影响新条件。observed／masked／secret／readonly／notapplicable不接受普通值注入。继承来源与实际值分开。代理属于当前连接的高级设置，继承／禁用／覆盖及条件URL由Core保存、回读；Google登录不另建网络表单。批准稿已有继承／直连，覆盖地址另来自现行AI-CFG01及既有客户端能力；其新增字段／状态是独立实现增量，尚不能继承原整稿或独立视觉参照批准。

新增、编辑、保存、测试及选用分开。保存不切服务、不启用、不改当前句；统一浅层选择器只影响后续任务，保留范围与启停。缺配置／真实handler不可选。复制产生独立草稿并清除秘密和离线资源绑定；删除、共享影响及返回来源复用业务端口。模型列表只辅助选ID，失败保留手填及原选择，不能提升翻译／学习资格。已有四原生及旧generic列表wire，下一切片接已保存来源的显式发现；完整发现及配置生命周期仍需接通实测。

## 3. 配置、来源与实际存储

codec1／2及原seal语义保留；codec3使用独立严格ConfigurationV3，HTTP与offline联合分开，离线不造endpoint／model／key。局部EngineTarget与跨版SavedServiceRef分开；Inherit／Set意图和字段来源保留。旧源不能表达的事实不通过排序重编码或假实体恢复。

实际Store4沿生产004与专用writer验证原sealed、未GC来源及逐字段复制。connection-only内部证明保留无模型旧连接，不造模型。materialization、稳定control alias、FK、唯一、seal、head CAS与GC由Repository核验，调用者对象／hash不授权。codec3另核metadata／seal／canonical内容身份，旧codec不重解释。

Custom Origin可选site与自定义地址并用，按已审核R3存取；codec3 TypedV3去重与LegacyV2旧slot按各自合同处理。R4 connection-only、740项独立存储义务及原v1～3义务保留。独立Oracle、DDL容量探针与真实Repository分别验，不能彼此代替。

## 4. 协议、凭据与Google

execute／test／discover先精确lookup协议及profile，再选择实际材料。Key、百度／有道AppSecret、Google服务账号及用户OAuth是typed材料，不猜JSON种类、不当普通Bearer。SecretText有界、脱敏并尽力清零；Vault版本、pool policy、成员和lifecycle取用前后复核，Key不进入共享client默认header。

| 路线 | 执行合同 |
|---|---|
| Chat、Anthropic、Azure | 保留旧协议语义，精确注册后复用原有界transport |
| Responses | 独立responses及typed SSE，明确结束，stateless／store=false |
| Gemini原生 | generateContent／streamGenerateContent与终态检查，thought不进正文 |
| Ollama、LM Studio原生 | 独立路由与NDJSON／typed事件，不偷换为Chat |
| DeepL、Google Basic／Advanced、百度、有道、TokenHub专用翻译 | 独立认证、语言、签名、字段与错误协议；Advanced只收真实Owner短lease |

现reqwest保持代理、超时、禁重定向和响应预算；未知参数拒绝，不静默删除，不执行工具、外部集成或隐式服务端历史。md5仅百度互操作；ring／base64等按已审技术锁复用，License随包保留，不引入AI编排框架。

Google Broker复用PKCE、固定官方目标、有界loopback、Vault及真实SQL CAS。显式登录才打开系统浏览器，读取、编辑、保存不自动登录。桌面client本地格式核验不等于远端账号可用；内置client材料缺失明确不可用，不造ID。成功仅把opaque grant放回原编辑、标未保存，不自动保存／选用／启用。

真实Google资格必须有Gateway持有的非Serde handler见证、实际writer身份及profile／server／generation，SQL仅Weak登记。仅Broker、metadata、compiled supported或相同frame字符串不授权；观察句柄不得延长已销毁Gateway寿命。Task／Attempt／受控测试在dispatch、cache、mint／refresh和terminal复核原权威。实际token观察时间与保守expiry分开，refresh保留旧mint材料谱系。

取消落在grant已写入而Core尚未发布的间隙时，精确失效本次新grant；未确认清理不得宣称已取消。重启专用恢复只清旧server／generation、未引用且未激活材料，失败保留可恢复事实，不因无权限待清理项让取文客户端循环启动失败。活跃授权或物理工作未结束的阻断保留。真实Google账号另做live回归。

## 5. 保存服务测试与共享执行

保存服务测试是明确独立操作，不造阅读Task／Attempt／Binding。默认固定日语样句，或明确固定unit／revision／source hash与实际token；复用真实提示、上下文及术语。配置语义、输入和授权hash分域，不借fallback或实时UI改变被测对象；未保存编辑不得冒充已保存测试。

Core Controller准备起即持唯一OperationBudget和原Arc；保存策略只能缩短原预算，不重开计时。调用窗口／receiver消失仍监督实际终态；blocking I/O持physical guard，退出等实际drain。取消和成功提交共短fence；沿原actor内短终态事务进行验证与COMMIT互斥，禁止在fence内执行Vault／网络／文件／worker、外部await或长阻塞。实际wire前核durable replay，同ID异请求拒绝，同请求不重复付费。

Repository重读原来源、seal、control、pool／grant及原live操作，再提交终态／资格。观察到而未提交的mint只作负审计，不是active lease。多key只测一项显示部分测试；按真实被测版本、用途、输入、材料覆盖。相关历史流式核验，不能用LIMIT挤掉已知失败。取消不等于通过，probe不等于翻译。公开失败只含应用拥有的类别、阶段与恢复建议，不反射原响应、URL或凭据。

已保存来源模型发现复用原OperationBudget、同writer来源/控制/材料核验与真实Gateway，作为独立明确操作；不创建阅读Task、Attempt或测试资格，不推进凭据池游标，不保存、选用或启用。第一实施切片先交Core/Infra真实链与共享PhysicalJobs，Native/React接线及逐态参照另验。新codec3来源可不填实际model ID；旧codec维护路径保留，新增入口的兼容恢复仍需完成，不能用该切片关闭AI-CFG02。通用Core不依赖Google授权类型，普通模型发现不依赖Google handler或client配置。共享物理guard由真实closure/child持有至结束，停止准入与排空由原Owner决定；不能取消即关闭仍需登记的终态I/O。已完成身份与近期结果缓存分层有界，逐出payload不重新执行原operation，保留明确过期结果和零I/O重放语义。

Codec3沿WorkDispatch／ServiceTaskDispatch进入原Runtime。SQL冻结真实Task／Attempt target、选用／来源语义、epoch与资源／凭据权威；cache、retry、dispatch及terminal核同一实际对象。只改对应scope／purpose，不重写其他runtime pointer／epoch／hash。fallback来源闭包、授权、顺序与失败语义仍待闭合，fail-closed不表示该能力已交付。

## 6. 离线资源与执行边界

| 切片 | 职责及当前状态 |
|---|---|
| O1 | 已有纯typed refs／canonical／v2联合，纯形状不授执行权 |
| O2 | 已实现只读材料／compiled policy／Expected重建并限定独审，复用同一物理检查器；仍待公共关闭准入与真实drain，不授grant或loader完整度 |
| O3 | O3A已实现004 Unknown行／seal／head CAS、实际writer checkpoint及回滚并限定独审；O3B真实grant／Installed资格及完整GC仍待接，不由Ready造Expected |
| O4 | 待接v2与真实引擎材料核验，旧v1保留；合同针对支持路线，不额外承诺通用OS沙箱或逐文件封存全部系统库 |
| O5 | 待O2～4，Core私有短lease、组合Gateway与真实Task／Test／cache／terminal |
| O6 | 显式目录能力及Apple前台语言准备，取消／下载／独立重观察分别处理 |
| O7 | 分片独审后整树、实际Store、组件、Native及同包游戏 |

采用RTD01～07：单个显式授权外部根，不隐式复制、安装、下载或写根；key按UTF8最多1024bytes，文件含control最多16384，control32MiB，帧1MiB，route最多3hop。稳定runtime hash排除locator／grant／epoch／control自身；snapshot manifest含control行，两域分开。由实际封存bytes和compiled policy重建Expected，不信Ready、版本、RECORD、sys.modules或自声明Complete。

实际检查路径、regular／no-follow、预算、别名、hash和全集合；叶保护不冒称祖先race已解。PathBuf不是授权能力，私有Inspected材料仅表示检查时I/O事实，原Owner／writer／grant／operation与loader另验。Argos及Windows加载范围未证明保持Unknown；缺loader机制是实现缺口，不能统称用户无资源。Apple用真实SystemApiBound联合，不造installation或不可公开语言包hash。

Core私有非Serde短lease绑定真实来源/control、frame、原操作、grant epoch、安装／环境／snapshot／route／observation head、closure／policy／processing及worker build。dispatch、cache、terminal分别重读；撤权、换根、内容变化和旧Final不得成功。重启不复活lease；GC保护来自实际live Owner，不接受UI protected列表，不在SQL actor嵌套回调store。

四引擎沿现tokio与独立有界worker，不造云回退／业务队列／任意shell。商业、Argos或系统语言资源缺失单列live blocked，合法资源不随包分发。Apple显式用户准备后独立观察installed，不能在后台扫描／保存时下载或把本地取消描述为OS下载物理撤销。

## 7. 页面组合与独立状态

三处布局补充已获用户确认并按精确9源/manifest实施：历史筛选/正文同轨16px内容线、通用设置14px行内安全边（其组织/组距由新通用三组结构批准替代）、停止会话保留三个旧句动作并突出重新连接。无当前句不等于没有保存历史；实际binding有效时独立预览保留，解绑才撤出。真实客户端沿既有窗口/任务/配置端口，不复制原型辅助画布或定时器。新增strict source-authority只判源批准和有限描述符；真实组件、独立比较、签名包Native及整R90分别验收。

当前句、来源、原译文状态和完整常用动作独立首屏；浅层统一选服务。正文历史、全部提取与Hook诊断后置默认折叠，各自筛选／分页／跟随及真实摘要独立；重排不重建producer／词卡／其他查询。SourceText使用真实token／ruby，固定词卡继续解释旧unit，新字幕不覆盖学习对象。Main以HUD舒适基准18/24对应23/18分别派生原文/译文字号；独立自定义值保留，HUD紧凑模式不挤压Main，流式与完成字级一致，不回写Rust偏好。主窗不重复HUD/设置的分词控制，七项常用操作保留，单结果来源/目标语种/人工修正合并页脚，多结果逐项标明。原源稿Main没有富文本标注状态，独立参照缺口保持blocked，不关闭真实词法能力来凑图。

翻译设置定向往返新增仅translation的生成DTO和两窄IPC；Rust单调pending先于show与目标事件，TS先listen后query并去重；同页目标保留脏编辑/密钥/测试状态，普通openWindow与真实离页确认保持，不新增存储或框架。

手动页按批准原文编辑/动作/常驻结果布局组合，Header提供翻译设置，保留共享选服务的manual紧凑入口、现有三种源语言及真实ReadingContent。结果固定手动提交回执，兜底核手动sourceKind，Hook的gameId为空不转手动；保存刷新与真实任务状态/取消分开，不从缺失语言投影造目标，不清历史或用原型timer。原稿缺的紧凑选用、可编辑语言、富文本结果状态不自动通过。

HUD顶部服务选择器采用本次明确请求的间距修订：38px栏、上下6px；共享26px选择器与24px工具保持尺寸，原61源以单一末尾CSS组合而不覆盖父批准。新像素及Native分别核验，不提升完整产品门禁。HUD保留半透明、拖动、位置／快捷键恢复，仅正文滚动；菜单不重复常用动作。C35身份只陈述已核事实，当前帧、历史成功帧与原采集时间分别表达；独立collector由消费者订阅，权限／核验分开，恢复等新帧才更新，撤权清画面。

历史双查询、cursor及generation独立；replace失败保原成功范围，append失败保已有页；编辑禁全部导航并保focus／caret，定位后多页无漏重，不猜总数／水位。通用设置以新三组稿替代旧大表：标题轻scope与分词词典、阅读记录、本机偏好，字段mask/继承/草稿/CAS原逻辑保留，恢复仅draft/save生效。会话接收和翻译移回当前会话，通用必须以独立typed `session`意图进入真实会话页；原`capture`意图继续只负责选游戏，不作别名；导航必须覆盖真实MainWindow消费者，不能仅验发送参数；OS登录状态短显示/详情展开，读写回读独立，未知／错误不冒充关闭。原始批准V3五源按原字节留captured，永久入口仅变两依赖URL，迁移需原始像素逐图0差异；源批准不授产品像素或Native。

复用Select／FormRow／Button／Sheet和确认token，按当前设计组合，不恢复旧实体Tab。真实client、宽窄／长文／200%／主题／键盘／词法组合逐项验证，局部迁移不关闭整页。

## 8. 证据与放行

编码前verification逐文件登记Owner／需求／检查；新技术、公共合同、存储及跨层职责按影响审。原失败保留，最终非作者审核绑定整棵当前树及全部实际改动，局部handoff不能代签。

设计源、独立参照、真实React候选、正式包Native分别验；保留原DOM和PNG，零阈值／批准图不改。旧v1捕获器留历史身份，v2仅审定顶层坐标归零，内部文字／布局不改。809状态、原逐层／存储及五类负例不减，缺adapter／render如实阻断。

最终执行npm run verify -- --change implement-approved-client-plan，再以同一稳定签名包验SR、MO、Never7、サクラノ詩10th连续五点、译文、分词／词卡、预览及退出恢复，不测Ever17。其他Steam会话维持授权边界。缺真实账号／资源／桌面条件按对象blocked；编译、受控listener、HTML、旧包和旧游戏结果不能替代正式包准出。
