# B正式集成设计

依据：已记录的R39-A方案、phase-B-plan、真实组件两场景七失败断言，以及用户当前要求。

- M12唯一App CollectorSupervisor lazy spawn同exe最早worker分支，存活跨Hook收尾、页面、订阅数量；child独占AppKit、原SC filter、完整Win/native pre/post及SC/既有精确单窗后备。正常生产无A的300秒/120帧终止、默认截图/journal工件；真正App Quit精确收尾，不重启整个App。
- 原WindowPreviewService改私有代理/最多Candidate+Session两owner/global1在途及最新帧缓存，不在Main建capture_one定时线程。主窗binding读取只读worker/已提交目标投影，停止5秒主动Win probe；封面只消费已核验有效帧，Core一次fresh核验委托同worker明确请求，不用旧时间充fresh。不得持Hook/lease锁等IO；provider用Weak避免环。
- 子协议版本化并核workerInstance/source/capture/target/frame epoch/request/seq；保持source范围Native PID/start/CGID pin，target或worker换代映射到前端不可重用leaseId或严格新generation。candidate→session保留原filter与child、fence frameEpoch后下一次完整捕获才给session/封面；取消迟到只释放自身。最多2授权context+1无filter pending，1活跃采集，帧/控制队列有界。
- 保留已有自动可信映射及已有权限自动预览语义；只在无法安全映射时投影准确selection_required，由明确按钮在worker显示系统singleWindow picker。不得对所有目标新增自动弹picker，不在订阅effect中开选窗。取消/超时/失败保留准确逻辑状态，可在同一App明确重试；无需为重试重开Main。
- 用户暂停/恢复/解除关联/取消候选/更换目标是控制意图；React挂卸载、读失败、翻译/SQLite/Hook故障不是控制意图。Disconnect不Shutdown collector，真实目标/许可失效仍立即fence且不显示实时旧帧。
- 本地TS ApplicationPort有意改为start/select不暗装listener，attach返回独有unsubscribe closure，删除leaseId模糊detach调用。无Tauri命令/Core DTO变化。每lease最多4监听者，一个single-flight上游pull，缓存仅latest；旧unsubscribe/finally不得清新订阅，最后取消只停WebView拉取，不stop source。
- ReadingPanel同session/target可信绑定遇严格typed暂态读取拒绝保持binding/preview，不伪造verifiedAt；初次未知/真实unbound/身份变化/许可撤销需准确状态且拒旧帧。旧图片只能带原时间标最后成功画面。
- 原RBC27/43/46及81项保留目的。旧candidate effect卸载/异常隐式stop与新纯订阅权威不兼容：只允许将对应owner清理断言机械迁至CaptureSheet明确取消/替换/关联CAS的生命周期场景，同时永久断言effect取消不控制producer；保留旧红回执，不能删断言来绿。原会话不stop/afterSequence重挂/迟到隔离强度保留。新增RBC82–97真实组件/生产adapter场景。
- 原probe12/30秒预算保留，停止先fence并在既有3秒接口返回Stopped或cleanup_pending，只有实际drain后释放全局槽。Acks不是reaped；真正底层可取消Wine query与性能优化另案，不扩大本轮hook-client/Windowsbroker。严禁修改游戏几何/DPI/图形/TCC或为真实回归重启容器。
- 全局8MiB/4秒SLO未完成仍明确缺口，既有视觉source drift继续失败，不改批准PNG/token/阈值/技术锁/DB/Core/License。


### 选择回调观测与旧effect重试迁移

桥仅额外记录有界不含窗口内容的选择回调事实，当前未知stream安全拒绝并返回typed终态；旧operation回调不能影响新operation。既有selector/ARC/RunLoop检查不证明真实OS回调已到达，须新包实测，禁止直接归咎用户或宣布根因已证。精确新增路径与Owner见迭代卡。RBC72的原无限重建防护迁移到有界纯订阅读恢复与显式Owner恢复；effect不拥有producer的合同优先，旧用例身份及历史回执保留。RBC73继续原目标切换/取消的晚到隔离断言，不因该迁移弱化。


异步系统选窗等待与full-proof调度分离；精确Native transaction是否活跃由桥内私有受锁查询确认。Release先fence并保留retiring context，首次异步提交前的预留与最终真实drain共同构成槽回收门槛；未知/未开始不能被误认已释放，详见迭代卡的编码前授权。


## candidate20现场后的首帧契约与诊断补充（2026-10-24）

20的正式界面已经接入Hook并实际收到Artemis日文，独立预览仍无首帧，候选/会话超时事实保存。R42默认包统一verify57/58；唯一失败是原设计app.js冻结来源漂移、发生在PNG差异前，不能改批准source或阈值来通过。

Native尚未解析时同lease/generation/consumer的targetIdentityHash=null；首次经过worker完整校验的帧使projection与frame共同获得真实hash，这是正常单向解析，不是换目标。前端只在同context、两份真实hash一致、帧结构与单调序号合格时接受；已解析hash不可改变或被迟到null元数据/旧订阅清空。所有listener复用一个pull，初始未知状态不得变成跨目标豁免。

采集失败证据用同worker私有Status承接有界typed摘要，Native薄适配在macos/mod.rs，不新增公开DTO/服务。诊断绑定当前transaction/source/epoch/request，仅输出固定阶段/回调计数或枚举/白名单系统错误码/配置与实际尺寸。默认不保存画面或用户内容，原完整proof、单采集、预算、filter生命周期和compat fallback条件保持；没有首帧时准确呈现未取得画面。诊断自身不提升实机验收。


## 休眠后采集恢复与原始单窗filter（R45）

原始已共享的单窗口filter是授权范围，WindowServer的onScreen只是可见性观测，不作为已核验filter路径的截图前硬拒绝。仍须查询同PID/start/CGID并通过前后完整proof；自动定位无filter时原onScreen规则保持。允许系统SCScreenshotManager实际决定该次单窗结果，不扩大成桌面采集或新窗口。真实失效必须停止发布并保留准确原因。

图像编码或同窗口尺寸在唤醒期间暂变不等于授权身份失效；仅在同choice且每次重新完整核验、既有单flight及重试预算内恢复。权限/身份失效立即撤销，预算错误仍拒绝，无效图/元数据不刷新末帧。诊断包含白名单最终错误码/固定阶段，原Native初始错误与fallback最终结果分别记录，避免只看Native4误判。此次只修采集恢复，不引入Main/worker自动重启或系统休眠操作。
