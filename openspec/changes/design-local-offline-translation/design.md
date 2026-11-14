# 设计边界与复用

统一翻译业务端口按HTTP/platform_native/local_worker判别执行，Rust拥有请求/配置/取消/缓存归属；UI不直接调用翻译库。系统adapter、专用翻译worker、词法worker和Hook host隔离。离线引擎仅translation，不升级grammar/context_word。Apple传统/智能策略与OS/SDK可用性分开，Argos pipeline/生产运行栈与专用语言包须PoC、许可及技术锁审阅，传统DLL同位数和CrossOver单独验证。

整稿候选位于docs/design/first-product-v1/candidates/local-offline-20261030，复用原index布局/共享材质/自绘控件/图标，使用独立settings扩展。原canonical settings/index、techlock、16历史源、原PNG/差异阈值及11历史alias均不变。README和PRD指向新pending入口。

草稿、可用性探测、指定样句测试、应用、启用分开。安装/资源/版本/语言方向/直接或显式中转、取消迟到和多引擎共存明确。字段与迁移仅候选，不修改SQL/字段导出。OT01-16规划与本轮设计浏览器执行分层；真实断网/运行器/系统API/DLL/四游戏仍not_run。
