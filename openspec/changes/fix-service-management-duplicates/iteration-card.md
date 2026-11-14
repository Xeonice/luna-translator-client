# 迭代卡

| 字段 | 内容 |
|---|---|
| Change | fix-service-management-duplicates |
| 授权 | 用户指出真实服务数量异常；继续同包游戏/翻译回归 |
| 前像 | a0057daa1734e8db0f279976201bbaafb2ccc0c5b7b966ef00408c0432c67b48；2765源；R91 |
| 需求/设计 | FP-06/11、MC-08/12；approved guided saved models/local engines管理列表；不改批准图 |
| 模块/组件 | M04/M06/M13、C26/C27；UI只呈现Rust已给定精确当前根 |
| Owner | implementation:纯呈现与ModelConfig接线/unit；browser_tests:真实组件截图和旧fixture；Root:治理/文档/Native；独立review分离 |
| 允许路径 | verification.json精确路径；编辑前受控 |
| 验证 | TypeScript、Node负例、Playwright、source identity/边界、真实正式包、真实服务与游戏 |
| 完成条件 | 一实际资源一行；历史/绑定不变；同名不同ID不误并；无假通过，准出缺口单列 |
