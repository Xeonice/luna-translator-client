# SR-6技术设计

使用R54实际impact/runtime adapter和既有application port、UI入口及独立OS/SQLite/Host/私有trace观察。固定四游戏顺序SR中文、MO日语、Never7日语、樱之诗日语；不测Ever17。Source、包、安装、主PID/start、资源和nonce单一贯穿；由真实动作及机器断言决定结果，不以人工ack或旧包记录代替。真实系统SleepWake需发生本轮Sleep和Wake；Source无变化亦强制全9义务。此前R58视觉已知偏差继续独立保留；用户已确认更新紧凑设计源，但本轮明确选择暂不实现客户端新布局，不用新expected换取通过。

遵循R47技术合同、单writer/身份/许可/预算/代次/CAS和真实释放确认；不改变公共ApplicationPort、DTO/IPC、Core ABI/JSONL2或数据库schema。文档与测试伴随本片，结果不继承旧包。

R59采用原生CGFloat显式转换、无阴影截图及分阶段取证诊断；原preview_text唯一裁剪/当前句OCR、五句、真实游戏/主窗/HUD、退出释放与睡醒条件原样执行。新的16项原生纯检查和58项工具门禁仅为准备证据。
