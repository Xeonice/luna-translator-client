# SR-3 会话预览所有权与封面消费者分离

用户已授权执行整个既定重构计划，本片范围为：将reading IPC中的预览ownership/epoch/CAS及长生命周期协调提取为私有平台coordinator，将现有首帧封面loop独立为只读consumer。

公开命令和DTO、候选移交CAS、用户暂停、Hook状态、真实释放确认、Weak proof关系和原子封面提交顺序保持；封面不能启动/停止producer。

不混入剩余产品能力，旧视觉失败、正式包和四游戏实测独立记录。
