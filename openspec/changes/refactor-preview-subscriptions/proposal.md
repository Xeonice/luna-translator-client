# SR-2 预览订阅归属与传输状态隔离

用户已授权执行整个既定重构计划，本片范围为：将预览pull/listener/cache/transport管理从TauriApplicationPort提取私有client；修复旧context永不settle阻挡新context及metadata恢复假Live的已证路径。

保留全部原100组件检查ID与断言、每context single-flight、四listener上限、500ms正常轮询、权限/身份/代次校验及UI/公开端口语义。旧RBC91同步release场景仍须最大一个在途调用，不能改预期迁就代码。

不混入剩余产品能力，旧视觉失败、正式包和四游戏实测独立记录。
