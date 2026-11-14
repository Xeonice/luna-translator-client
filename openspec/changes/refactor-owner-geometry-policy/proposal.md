# SR-5 私有几何原因策略模块提取

用户已授权执行整个既定重构计划，本片范围为：仅将现有content_fit_policy纯分类器物理提取至owner_geometry.rs，保持其逻辑模块和单权威，不改变词卡关闭或原生geometry effect。

所有原函数/类型/可见性、owner实例/epoch/user_generation/revision/CAS与14个策略测试原样保留；四个pub类型仍在window_manager，原生apply/readback及持久化留原处；不修改浏览器resize/blur监听，不改游戏窗口位置/尺寸/DPI。

不混入剩余产品能力，旧视觉失败、正式包和四游戏实测独立记录。
