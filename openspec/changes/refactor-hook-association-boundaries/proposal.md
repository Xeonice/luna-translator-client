# SR-4 Hook facade内部关联、身份证明与journal职责提取

用户已授权执行整个既定重构计划，本片范围为：将Candidate/发现/可信窗口关联读、preview freeze/proof和纯journal适配分离为三个私有模块，保持公开facade及会话原子边界。

Core ABI1七回调/JSONL2、同位数broker、public HookBackend trait、session expected SID+fulltarget compare-clear、FIFO relay/barrier、真实释放ACK及数据库事务保持；不按标题或最大窗口猜对象。

不混入剩余产品能力，旧视觉失败、正式包和四游戏实测独立记录。
