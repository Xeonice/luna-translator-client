# 验收闭环技术设计

1. 复用current_design_alignment/current_product_coverage的现行义务与release_gaps及既有package/runtime inspector，不复制业务注册表或放宽实际层。
2. 四份事实分开：source声明状态、source保留DOM/固定CSS/fonts的独立测量、注入产品偏好、产品保留DOM/资产的独立测量。不能从producer style JSON或prefs假称独立算CSS。资料不足返回missing_render_evidence/incomparable。
3. 测量记录原文/译文区域有效字号CSS px、注音/词性可见结构、字体ready、desktop/document/visualViewport/DPR/zoom、window/client rect/media/grid与捕获scope。禁页面JS、禁外网，精确allowlist并绑定原件/命令/nonce/源码前后；fresh replay核对。不能改变产品prefs迁就截图。
4. target及fullPage:false viewport不满足whole-client/整页义务；原1600复合画布不得裁剪重标为新批准参照。现有4参照与原PNG保持原字节/范围。
5. 从现有readiness、实际执行和独立资格衍生completion，不接受手填passed或局部检查计数。product_completion=incomplete时next_stage_allowed=false；需要产品完成的门禁实际拒绝。
6. 原5类越界守卫、源身份、编译和全部旧测试保留；负例包含伪eligible、相同prefs不同render、context不等、组件冒充整页、旧包/进程代替新包、blocked/not_run被绿化。正向fixtures仅验证工具能力，不替代产品实测。

当前产品必须仍未完成；此迭代不进入UI改造。详细只读审阅依据见ignored review/verification-closure-plan.json，实际回执另存，不能将方案当通过。


接线验证采用同源签名包运行既有current4实际适配器，保留34个原语义断言与4个严格像素结果，再附独立有效渲染资格。打包仅验证工具接线，不安装、不修改产品UI；实际产品对齐预计仍失败，不能把失败改标passed。
