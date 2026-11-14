# 迭代卡：全量模型服务设计文档

- 工作项：specify-full-provider-registry-design；类型：tooling/documentation。
- 需求：FP-06、FP-08、FP-11、FP-12；AI-CFG-01/06/08/11/14/15；MSD-001～003。原需求ID不重编号。
- 设计依据：first-product-v1/settings.html、interaction-review.html；已有确认保留，本轮新多字段认证/OAuth状态尚需后续具体页面及产品验收，不宣称批准。
- 模块/组件：M05、M06、M13、M15；C26、C28；沿用既有模型配置与Port，模块矩阵不更改。
- Owner：Root负责6MD、目录JSON、OpenSpec与最终采用；工具Owner仅3个Python脚本，独立架构审查/工具审查分别留回执。协作者不得覆盖他人内容。
- 允许路径：verification.json精确列举；不授权产品代码、native、SQL/DDL、锁文件和既有图片。
- 前提：现有R68来源快照、13/21全量用户指令、TMT排除、既有文档及官方协议资料。本轮检查无需服务Key；后续实测须逐服务账户、额度/地域/模型ID、Vault授权、GoogleOAuth/SA及四游戏可操作环境，详见主技术稿5.11。
- 完成条件：旧文档同步、目录矩阵一致、实际守卫/独审及同源HTML、统一工程门禁回执、精确delta和基线采用；产品验收仍not_run。
- 验证：npm run verify -- --change specify-full-provider-registry-design；report脚本--check/--output；32项守卫。实际结果单独维护在.build/provider-docs-update-20261030，不预填结果。

维护发现：verify.py新增两项文档检查后，M15唯一来源引用须精确重绑整文件hash。仅改implementation-status.json该1个sha256及原render派生engineering-modules.csv该1个evidence_refs；行号/quote/所有行为与状态/历史/包身份/产品源集合不变。该最小机械映射是本轮额外精确路径，不是新的产品状态核对或通过。
