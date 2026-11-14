# SR-2技术设计

私有PreviewSubscriptionClient注入latest invoke闭包；权威projection/frame缓存、每context flight和本地transport notice分存，消息交付时组合兼容View。旧A flight只清自身，新B具独立一次next-task kickoff；不等旧A finally，不增加重试或放宽预算。当前active owner仍同context单飞；metadata-only成功不能伪造新图或将错误后旧缓存升Live，等新有效frame恢复。真实TS依赖loader读取新私有模块，外部Tauri invoke保持受控，禁止mock新实现。原RBC91/92新增子向量覆盖已证缺口。

遵循R47技术合同、单writer/身份/许可/预算/代次/CAS和真实释放确认；不改变公共ApplicationPort、DTO/IPC、Core ABI/JSONL2或数据库schema。文档与测试伴随本片，结果不继承旧包。
