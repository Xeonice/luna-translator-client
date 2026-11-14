# R39-A 迭代卡

2026-10-24，用户要求定位周期预览并指出独立进程/订阅边界。Root按既有缺陷修复授权实施本技术PoC；产品能力不扩展。

- 依据：FP-03/04/12，当前整稿ansi-preview-recovery.html#native-pairing及window-preview；M02/M12/M13，C13/C14/C35；保留已确认视觉。
- Root独占规范/基线/安装/GUI与真实游戏；Native Owner独占verification列明的8个产品路径与永久测试；其他协作者只读审阅或独立.build证据。不能覆盖或回退别人变更。
- A精确范围：main.rs、platform/mod.rs、window_collector/{mod,protocol,worker}.rs、macos/{mod.rs,bridge.m}、app-infra/hook_backend.rs。只从现有M02抽复用的纯冻结context核验，旧wrapper行为保留，不复制状态机。
- 最早worker分支绕过Tauri/bootstrap/profile/DB/Core Host/语言/WebView；父以绝对同exe直接spawn，私有必要FD与随机握手，裸参数直接拒绝。无公网/本地TCP/HTTP服务，无新框架/依赖/公共Tauri DTO或IPC。
- picker/原filter/采集/完整proof归worker；不跨进程传闭包/对象/opaque指针，不用裸CGID重建原授权。仅明确PoC意图开启，默认普通启动不运行新PoC，不自动弹picker或重启循环。
- 父身份快照只能来自已授权目标并由worker复核；child只读probe，不能注入/断开Hook。所有FD最小继承，无DB/凭据资源传入；队列/帧/超时有界，慢订阅者不得阻塞控制或采集。
- 依赖：同C514…开发证书/安装路径、可见桌面/真实游戏/系统选择、已验证Core资源与既有broker。TCC归属须实测，不能假定已授权。当前16主进程31844及游戏88536/2148保持现场，不改宽高/DPI/图形、不重启容器/Steam、不登录接管。
- 完成A：签名同源包、真实双PID，worker自身窗口选择/权限/多帧，父正常轮询与UI卸载不改worker实例和captureId；错身份/权限撤销/系统停止/取消迟到/慢消费者/父退出均真实核验。未执行not_run，外部条件缺失blocked。
- A只限PoC，不把旧producer和PoC双采集当正式方案。B需A回执后补卡授权，主App最终必须停止周期probe并只读worker快照。原视觉失败、全局8MiB及历史运行缺口不提升。


A阶段外部proof收尾限定：复用现有hook-client执行器的12秒verify与最长30秒inventory调用，不为PoC复制执行器或扩大修改路径。冻结context限定当前目标环境，停止标志在每次外部调用前后检查，已接受Stop立即停止后续采集/发布，但StopAccepted/Draining不等于Stopped/ChildReaped。当前只读调用按原有界期限完成后精确释放filter并回收自有worker；父正常退出等待最终收尾，不能留下probe进程或杀Wine容器。实际最坏总时长须由实现和测试核对，不提前承诺3秒。此为A技术PoC限定，B集成前须评审并落实面向产品的有界取消/退出语义；不把此预算改写成现有产品stop合同已通过。


A实现预算登记（待实际测试核对）：父等待真实已授权目标180秒；显式采样最多300秒/120帧；测试工件仅4张JPEG环槽（JPEG原上限524288字节，合计≤2MiB），events日志≤2MiB且≤4096条，累计JPEG写入≤60MiB；不是全局内存8MiB已实现的证据。父/子停止区分Draining/Stopped/Reaped，父有界drain40秒用于覆盖现有单活只读probe≤30秒，实际正常App/macOS退出必须等待可观察回收，不只依赖Drop或进程exit。若8源不足接入现有退出权威须先报Root，不可越界加文件。此预算只适用于A技术验证，不暗改正式产品原有stop接口的3秒超时/cleanup pending语义；B仍须明确集成。


UI订阅缺口的先行红证明（未改UI/原测试/门禁）：`r39-a-candidate/ui-subscription-red-proof/latest-run.json`记录一次真实exit1、2场景/7失败断言、执行错误0，源码前后相同。真实ReadingPanel收到AppError-shaped capture_cleanup_pending拒绝后卸载preview并清图，fixture会话/服务lease仍有效；真实TauriPort同lease第二订阅覆盖首订阅，旧detach还中断新订阅。原RBC27仅fulfilled window_proof_busy+starting，缺先成功帧后的真实拒绝；RBC43/46没有同时双订阅。前者是现有错误分支回归缺口，后者是本轮纯订阅架构新增能力缺口，不能冒称旧版已支持。

B必须将这两项迁入真实组件/adapter永久验收，再补跨session/target/worker代次迟到、旧unsubscribe、新订阅、无订阅、慢消费者、Main反复轮询与worker存活/帧序列的组合检查。A的传输测试和双PID系统验证不能代替B组件或最终同包游戏；上述红测试只是受控组件，不代表已复现所有实机stale的唯一原因。


A源码冻结核验（2026-10-24）：Native final-02/run-01实际Desktop121、infra50、Clippy、ObjC、Native31/mapping、debug build及6种拒绝worker模式均按真回执执行；这不替代签名Main→worker TCC/picker/真实连续帧。一次edition2021递归rustfmt造成11项范围外格式漂移已保留失败现场，由Root以原hash精确恢复11文件；最终全product闭包相对原R38只剩批准5旧+3新8源，原snapshot/预期/门禁没有刷新。final-01仅原时点有限测试，不可冒称最终scope通过。

正常退出资源与证据分离：先有界收尾并精确ChildReaped，再以总1秒等待末尾status-reader和journal barrier；已reap而证据失败只标PoC evidence_failed，不能拒绝资源安全的App退出。Reader迟到/日志消费者中断永久负例均需实际执行；正常App生命周期集成仍由A正式包及后续B separately核对。


R40-A仅测试映射修正：R39统一verify真实拒绝required-product-execution，原因是本change漏登记原生窗口代码所要求的真实产品截图adapter。补登记既有product-ui-capture-v4，不改adapter实现/参照PNG/设计源/token/阈值，也不把预期canonical视觉source drift当通过；R39第17包及其失败保留。只修改本verification映射与本卡并逐字归档R39权威，产品8源码和所有459产品源、497引用、既有catalog/历史/批准/运行/发布均保持。新同源候选18另实跑verify与签名系统PoC，不继承第17包为真实窗口通过。
