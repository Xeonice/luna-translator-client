# SR-3技术设计

DesktopState注入Arc<SessionPreviewCoordinator>，内部唯一Mutex<PreviewOwnership>，不持AppHandle造成managed-state强循环。IPC保留角色校验/参数映射/薄委托，commands解关联改用epoch方法并保留preview_state/preview typed锁毒错误；其他owner错误desktop_state/desktop仍原样。锁内只取/转移租约意图，真实service control和cleanup ack保持既有顺序；原封面loop迁SessionCoverCoordinator，传入只读SessionCoverConsumer能力（不暴露start/stop），保留外部proof先行及service→capture_lock→Hook lease→短SQLite commit原子链。平台不得反向导入ipc。

遵循R47技术合同、单writer/身份/许可/预算/代次/CAS和真实释放确认；不改变公共ApplicationPort、DTO/IPC、Core ABI/JSONL2或数据库schema。文档与测试伴随本片，结果不继承旧包。
