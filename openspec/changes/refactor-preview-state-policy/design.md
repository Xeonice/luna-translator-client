# SR1设计

WindowPreviewService保留Mutex/Condvar、Lease资源、collector/Native effect及外部API。私有window_preview/state.rs只容纳无I/O、无Tauri/AppKit句柄的纯决策/转换。保留collector_frame合法身份形成及finish_capture锁内二次核验；worker frame序号与service projection序号分别管理。

Rust完成proof可在首图前形成合法身份；这不代表Live或图像。Pause在CAS/fence后允许delivery envelope generation迁移，但实际payload/contentHash/capturedAt不变，且不冒充资源释放。候选移交按既有实现表征，不能假定所有转移均保图。

先实跑既有完整Native维护测试，再增加必要表征反例/提取并独审。所有旧断言保留。架构、当前阶段和源码清单只同步实际变化，原PRD/批准图/技术锁/数据库不改。
