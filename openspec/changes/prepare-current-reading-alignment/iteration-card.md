# CP00-A 迭代卡

| 字段 | 内容 |
|---|---|
| 身份 | prepare-current-reading-alignment；父FP1-20261031-R80，2203源，8b3a38ee897fffdfd9dbd13481b55e8b53d36dad9eb6a5d42d11814253d2bd91 |
| 授权 | 当前用户“直接开始处理…阶段门禁和验证都完成后再进入下一步”；阶段依赖优先，先补测量基础 |
| 追溯 | FP-04/09/11、GD-01/02/03、RW-17～23/26～30；CP00-A/CP08；M15/QA，复用M13/C16/C17/C36 |
| 设计 | complete-guided-history-20261031，61源已获批；guided?scenario=reading，当前句及三数据集；参考候选、产品验收独立 |
| Owner | Root质量/治理/文档；reading_capture_implementation负责唯一Node runner；非作者agent审阅 |
| 精确路径 | 编码前ownership.json冻结；verification逐项列出，不含apps/crates/native/设计源 |
| 依赖 | Playwright/core/Chrome、既有Vite正式组件测试入口；没有Git，完整source snapshot |
| 验证 | PNG/输入/视口/nonce/源identity/命令/外网/实际DOM；篡改与错目标负例；完整verify、TS/Rust实际编译 |
| 完成 | 只读取证工具四变体真实执行，独立校验、反例、独审通过。若产品/阶段依赖仍blocked，停在门禁不进入UI实现 |
