# CP-01实现设计

1. Rust与现有ApplicationPort为业务权威，不增加HTTP/SQL/FFI或新DTO。只重组真实reading三个生产文件，复用SourceText、TranslationTaskStates、WordCard、SessionRecordPanel、WindowPreview。
2. 独立当前卡始终位于历史区前方，不属于任一折叠内容。词卡点击捕获不可变unit/revision/token；下一句或迟到翻译不改变已固定词卡和学习对象。
3. 三details默认关闭且相互独立。记录面板稳定挂载并保留状态；各自查询/范围/分页独立。可按展开/active暂停后台刷新，但不能卸载清状态、丢错/游标或停止预览。摘要必须来自真实SessionRecordPage/Completeness，不编造warning数字，不将未查询当零。
4. 当前卡依现有translation projection呈现等待、部分流、失败/取消/完成，不能回归闪现“尚无译文”。使用真实preferences、词法identity与既有偏好命令，不能仅加装饰开关。
5. 样式复用现有material tokens和SVG族，阅读优先。预览是稳定独立侧栏；窄窗可自然重排，只有指定records容器局部滚动。无虚假服务picker（CP02/06/07另做）。
6. 新句、折叠/展开、分页筛选、历史条目和词卡有真实React测试；原测试Tab入口按批准设计退役迁移，不删除失败断言。Playwright保存dark/light/narrow/200%候选，原批准图不改。
7. 本轮新用户指令只覆盖开工顺序；现有完整对齐gate仍执行并记录所有缺项，不将开发阶段实现等同准出。明确报告实际代码、编译/行为通过和未通过的产品/包/游戏验收。

## 新增滚动行为测试的实际量测

首轮CP01新增旅程在同record92/范围/筛选不变时测得offset -13.21875→-13.15625，相差1/16 CSS像素；原失败保留。经独立非作者审阅，浏览位置不变量采用实际devicePixelRatio换算后逐侧round的物理栅格坐标零差异，且record identity、loaded range、cursor、filter、follow、viewport与DSF仍逐项精确不变，原始DOM浮点和差值另留。这校准新测试的量测单位，不修改产品坐标，不使用任意epsilon，不改变任何批准PNG的零差异阈值，也不把行为位置通过当作截图通过。真实栅格位置变化/记录身份不一致仍属于产品缺陷。

## 新产品回归接入

新增CP-01真实React旅程必须通过既有product_execution登记执行，Root提供鲜活nonce、固定命令、完整源树前后身份、实际命令时间与原始log；独立检查具体11个case、正断言/零skip、原始截图与四变体几何、构建和源码来源。200%遮挡、hidden-main/在途分页锚点等进入统一verify，不只留手动报告。该有限组件adapter不获得完整current-design或packaged_native能力，不改变原PNG差异阈值和准出缺口。

## 实际缺陷的回归约束

200%字号按真实容器剩余空间换栏，预览沿原实例与租约保留，不按zoom特判。隐藏期间older页完成同时覆盖折叠和切回游戏库两条路径；保留序号、文本、范围和查询owner，恢复可见后比较实际DSF下相同物理像素位置，原始小数偏移保留。

新增有限真实React执行器接入统一verify：nonce、完整前后源码、实际构建日志和sourcemap、11项逐项断言、6张PNG及四变体原始geometry/action命中独立核验。缺失anchor/scope身份、零尺寸预览、文字溢出/遮挡、隐藏页跳行或旧回执均拒绝；该执行器不授予批准PNG、完整794状态或正式包/游戏能力。

## 打包预备资源的显式补充范围

首次实际打包在预备阶段因language-worker暂存二进制与R81指纹不一致而失败，未执行Tauri构建/签名/安装。保留旧二进制和首轮失败，不以改旧债务或忽略指纹处理。该暂存worker是当前已锁Rust源码经固定cargo release构建的派生产物，非手写业务变更；精确追加仅apps/desktop/src-tauri/resources/language-worker到M07/JA及打包Owner范围，先独审构建日志、Mach-O及真实stdio对照，再重冻R82。该资源预备补充不改变Rust/worker源码、协议、语言资源及License；原生/四游戏结论不由此提升。新包必须在稳定预备输入下重新完整构建。

## 完整门禁首轮发现的后续修复

首轮统一verify在稳定cab源上真实执行后，发现SRF-READING-VERSION所需六检查漏声明，以及M10需要真实storage v2 adapter；追加声明并实际执行，不移除M10或绕过生命周期规则。另全页影响计算在三个已退役Reading页签从AST消失后漏掉其负例义务：修复依赖闭包，使缺席的退役surface继承实际父页依赖，保留全部原状态ID与拒绝规则；调整守卫测试的旧导航前提，明确构造退役前fixture并再次删除验证，不把“已不存在”作为删除成功。精确增加两个quality程序/测试路径，其余批准源、PNG、状态总集不变。

## 必须修复的实际运行时文件锁失败

第二轮完整统一verify在稳定f35源上通过100+11组件、39守卫及真实存储v2，但默认并行infra实际报shared_pending_survives_reopen_and_expires_without_spawning的WouldBlock。真实Rust/fork/UnixStream握手已证明子进程继承文件描述符使父仅close无法释放锁；既有ProfileLease的owner PID/显式unlock模式可参考。精确追加game_launch.rs与hook_backend.rs两个Rust路径，M01/M02/M12/M14协作，采用窄PendingLaunchLease（private File/PID、成功加锁才构造、仅owner Drop unlock、无Clone/Deref/裸File导出）；reserve只接guard，保持discover→already_running→reserve→run及全部早退的原作用域。保持30/10/15秒期限、锁路径、错误码、公开DTO/DB及只连接已运行游戏的客户端产品边界。原失败断言全保留，并加真实guard/fork握手；默认并行完整infra和所需Hook-client/target-probe检查必须实际通过。V2先在ignored补丁独审；源应用后原f35包/回执仅历史身份，须重新冻结/复签/完整验证/新包。
