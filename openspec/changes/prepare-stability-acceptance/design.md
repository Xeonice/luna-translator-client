# SR-6-PRE技术设计

依据已完成的sr6-minimal-observation-review，仅提供有界非阻塞try_send元数据记录。业务锁内最多复制有限标量，释放锁后入队；JSON/文件IO/rotation仅专有writer线程。禁止帧内容、台词、译文、词典/模型响应、凭据、完整路径/标题/自由错误。guard accepted/service committed/server delivered/UI rendered明确分层，worker/service序号分开。掉落/写失败/轮转必须有health与watermark，缺证据仅阻断验收，绝不能影响控制结果。不等待writer作为退出前提。

依据impact-guard-analysis与sr6-runner-analysis和minimal-observation独审。preflight由真实snapshot/Git diff与权威规则计算影响；postflight独立读原始检查日志/组件/Native/runtime回执闭合，未知关键路径或缺检查拒绝。运行器仅通过普通正式客户端、独立OS/SQLite/已有Host/私有metadata观察取证；人工barrier只确认动作完成，不能提供pass。实际PID/start/boot/code-sign/source/resource/nonce/时间/序号/帧变化/日志水位核对；缺观测明确blocked，绝不自己填绿灯。runtime分支有独立执行预算和真正registry dispatch，不落入Cargo parser。

视觉测试宿主只通过公开候选projection/owner intent及prepareSessionPreview准备独立同身份JPEG；全原check ID/77要求/37状态保持，旧off不可达明确失败。

遵循R47技术合同、单writer/身份/许可/预算/代次/CAS和真实释放确认；不改变公共ApplicationPort、DTO/IPC、Core ABI/JSONL2或数据库schema。文档与测试伴随本片，结果不继承旧包。
