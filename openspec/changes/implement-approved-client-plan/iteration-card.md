# 已批准计划实施迭代卡

| 字段 | 当前内容 |
|---|---|
| Change / 父基线 | implement-approved-client-plan；FP1-20261105-R90；完整parent source map ca73010b4c86311cbc2901c7e939aeb19dc137abd64a8b732989876a0fc9f7fe |
| 授权 | 用户确认完整计划并要求执行至完成；设置采用A1000×720，其余容器方案采用；另已确认三处布局及通用结构V3，授权修正并重新升级客户端，按同签名新包验证交付；未授权Git提交/推送 |
| 需求与验收 | FP01～12、AI-CFG/RW/HP、ACD001～011、CP00～11；13协议/21预设/4离线引擎，不含TMT或Ever17 |
| 设计页面与状态 | guided+R79完整61源、C35五项补充；C1 Main/历史1040×720、设置A1000×720、窄760×560；809适用状态，参照/生产/Native分别验 |
| 模块与复用 | M04/05/06/10/13/15/16；Rust业务权威、窄Tauri、独立hooks-core/collector/语言worker；现有Repository、任务调度、SourceText、Select、Vault、版本化配置复用 |
| Root职责 | 通用结构批准/可审计路径迁移/现行PRD技术测试映射、通用UI实施及受控完整继承/导航/启动状态检查；Native发现session误接capture后，明确新增MainPageIntent::Session并经生成器同步TS，MainWindow按三意图显式路由，保留Capture原语义；实际消费者与标题字重/字体同态测试先红后修；DTO构建引起的单个暂存language-worker二进制由真实release/preparer派生、逐字保留前像并在冻结前精确准入，原worker源码/资源manifest/数据库不改，同证书正常退出替换与Native观察； 当前会话C1容器/紧凑服务行修复、Main角色字号纯派生、手动页需求/参照缺口及文档/测试登记、用户请求的HUD顶部间距修订及同签名包验证、固定实时UI审阅流程与准确状态；共享Runtime、保存服务测试Controller与同源操作注册、协议网关、组合根、跨层端口、文档/验证闭环及最终整包；不通过替换旧实现绕过已有任务/取消/缓存权威 |
| 并行职责 | UI实时预览Owner仅负责新dev-only iframe入口/启动器/同数据样例/隔离HMR验证，不改原harness；阅读UI Owner限ReadingPanel与阅读CSS及新增隔离测试，保留七动作、来源和词法边界，不改存储偏好或HUD布局；手动UI Owner限manual页面/CSS、共享选择器manual隔离外观及新增真实组件测试，保留原端口、源语言、来源与固定结果；Google/模型发现Owner负责已准入授权叶及已保存来源发现Core/Infra切片，并先提供唯一通用PhysicalJobs供离线复用；Native/React发现交付待单独逐态准入；离线作者负责O1纯契约及O2只读材料/compiled policy/Expected重建，O2复用同一物理检查器（不授grant、loader完整度或租约）；SQL离线Owner仅负责O3A真实Unknown材料/原actor checkpoint/CAS/回滚/引用保护；来源作者负责来源纯规划/保存测试请求和Provider叶；定向设置导航Owner负责准入12叶的typed DTO/窄IPC/pending/接收端/受控测试，不改普通窗口语义；各切片独立非作者复核 |
| 精确允许路径 | verification.json逐文件声明；新路径先登记；并行者不覆盖或回退他人改动；未授权新库/DDL/框架不得引入 |
| 前置与依赖 | 目录/codec/存储→真实授权与资源→执行/测试/细粒度选用→完整引导/共享选择/页面组合；CP00/08逐片准备，不能等最后才补 |
| 外部依赖 | 用户暂无额外账号/Apple语言包/Argos或商业资源；只使对应真实用例blocked，不降低实现范围或将探针当产品通过 |
| 验证方式 | 原失败保留；UI修改从固定loopback HMR入口持续审阅，实际client viewport独立于外层工具栏；实际Core/SQL/Vault/listener/子进程、Playwright整页/严格图像、Native、统一verify/五类负例及同包四游戏；精确源码和真实包身份不可混用 |
| 完成条件 | 所有承诺功能/排布/异常/适用层证据闭合、整树非作者审查和准出通过后结束；当前incomplete，next_stage_allowed=false |

## 当前风险与阻断归属

旧codec1/2来源只能验证其实际可表达的复制字段；不可恢复原字节顺序的Legacy1不得用排序后DTO重算seal。物化控制链与实际复制字段都需核对，检测循环不以任意深度代替。

Google实际请求须绑定真实task/attempt或受控测试操作，不能预写假pending/passed测试记录取token；token轮转记实际旧mint材料。离线目录扫描、完整runtime文件身份和资源租约必须先闭合，已有四引擎叶不能直接授予可选性。

技术增量仅采用已获独立审查的精确范围，见technology-lock对应adoption；不刷新父基线、legacy-debt、批准PNG或token以掩盖差异。完整源码审查、809状态和正式包验收不由局部采用继承。原R87等失败及本轮所有红/绿回执继续独立保存。
