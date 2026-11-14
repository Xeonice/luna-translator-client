# 技术修复

## 依据
FP-06/11、MC-08/12，批准guided-config.js:164-165，M04/M06业务权威、M13呈现、C26/C27服务配置。独立架构审阅见.build/runtime-game-translation-20261109/service-management-fix-design.md。

## 方案
采用受约束B：纯呈现函数读取Rust已给定的draft精确refs及三个selection exact refs，按logicalResourceId分组。当前管理集合以draft为主；当前使用旧来源仅以Rust selection事实补充。不解析全部applied alias，不按名称或revision文本猜最新。rawRows继续完整供fallback/editor/选择器使用。用途来自实际声明/绑定，精确源及逐用途验证状态保持。

## 边界
不新增Rust/DTO/端口，不删库、不改immutable历史查询，不新增版本选择交互。缺任何严格视觉或Native执行证据仍阻断全产品准出。
