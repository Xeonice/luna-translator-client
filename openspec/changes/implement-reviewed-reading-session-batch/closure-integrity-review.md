# 独审发现的接收收尾与完整性修复

CR-IMPL-01/02 成立；方案已由 Root 独立读取后批准在本批范围内实施。候选方案 SHA256：`052ad2a842cafd8c6f6a8b4ceef469fce2d93a6162e18724cec1dfaea6e893ea`。

新增仅内部 HookBackend stop-and-drain/FIFO 持久 ack、Client 生产线程生命周期与已接收队列排空、SQLite writer 外 pending unknown 重试。不得改 Core ABI/JSONL/DLL，也不得用一次空队列或超时当作无生产者证明。closing 禁正文/模型但保留事实；关闭/导出前须完成持久化，失败明确拒绝，不假 complete。

额外窄所有权给予后端 Owner：hook-client/src/lib.rs 及相应真实测试。sticky loss 仍拒绝正常消费，已接受的缓冲事实另外有界排空；正常订阅恢复不能制造无接收空隙。原架构、冻结数据库设计和外部 IPC 不变。必须用真实队列饱和/结束期间晚到回调/正常断开与重开验证，独立审阅后才封包。
