# 模型默认模板真实闭环

- 需求：FP-06/08/12；AI-CFG-02/07/10/13；MC-07/08/12（对应子项，不宣称整项已验收）。现设计 `settings.js` 模型专用模板/恢复用途默认和复制模型，当前细节交互未完全表达逐用途选择，新增状态仅作为独立产品候选，不修改批准稿/token。
- Owner：configuration_lifecycle_impl。模块 M04/M05/M10/M13；组件 C26/C27/C28。允许 contracts、domain/model_config、application/request_plan、queries/configuration及copy命令dispatch、ModelEditor/ModelConfigScreen及内部编辑器、generated DTO；真实测试新增 model_default_templates。其他协作者的history/HUD/Study/SourceText/visual/design-port不修改。
- 契约：Model.defaultTemplates按purpose映射稳定templateId，旧序列化缺省为空；持久化使用既有 model_default_templates 四列，禁止把表列覆盖当功能通过。模板compatibility.modelConfigurationId定义可选专属owner，缺省共享。相同owner+purpose下名称不得重复，引用必须匹配用途/模型作用域/已声明兼容条件。
- 请求：采用引擎→用途→模型默认→内置；游戏专用设置按已有PRD继承层保留全局用途模板来源，具体规则以根任务确认说明为准。Rust解析，UI不自行编译请求。
- 复制：typed CopyModelConfiguration仅复制已保存草稿，expectedHeadRevision CAS + operationId幂等；新模型参数独立、专属模板独立ID/owner重写、共享模板仍引用、共享连接仍引用。不应用或发网请求。
- 验证：真实Repository save/load/apply/restart +实际RequestPlan；覆盖默认优先级、跨用途/跨模型错配、重名、旧JSON、复制独立与共享引用、CAS/幂等、模板修改使请求语义失效。UI沿既有控件提供模型默认/恢复及复制，实际组件行为/截图由后续产品候选检查记录；不修改既有视觉门槛。
- 完成条件：真实测试/编译通过，报告源码和结果，独立审阅与正式包另验。无提交/推送。
