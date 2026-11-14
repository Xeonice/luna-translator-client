# 单窗截图取证设计

## 职责与边界

Owner为QA原生证据设施，Root独占Main整合及authority，AP独立审阅；协作者只写ignored候选。需求追溯FP-03/04/12，M12/C35。Core ABI1/JSONL2、私有collector、产品UI、DB字段/关系/迁移、Host与语言worker均不改。

## 生产路径

在原5秒有界 `screencapture` 调用中增加 `-a`，保留 `-x -o -l` 与实际指定CGWindowID。对实际解码CGImage的像素宽高和本次 selected CG bounds做有限正值及严格等比例核验；只有有效canvas才调用全图OCR与后续AX preview裁剪。不固定1x/2x倍率，不加入epsilon。

拒绝时保存已有PNG、实际像素尺寸、JSON安全bounds摘要、真实阶段与reason；NaN/Inf或极值派生比例不得破坏JSON输出。记录实际argv与排除附属窗/阴影的请求，明确 `actual_canvas_origin_observed=false`。原低层previewPixelRect独立x/y映射及负原点/Retina测试保持。

尺寸一致仅证明比例一致。无法由尺寸推出真实原点、所有权、排除同比union或内部叠加；`-a`效果需要真实系统实测。严格Double交叉乘积会保守拒绝分数point/像素取整不一致，不通过修改oracle或容差规避。

## 验证分层

沿原48 guards编译运行真实生产Swift纯函数，旧16 native ID顺序保留，增加11检查：2x/1.5x合法、HUD/词卡union拒绝、独立词卡合法、非法bounds、有限乘积/scale溢出、NaN/Inf JSON安全、拒绝不调用OCR、原点未被宣称。测试使用generated CGImage，明确不是屏幕截图。其它47测试及11个辅助方法AST不变；旧harness仅增加检查和加强argv期待。

后续实际 parent/child 各自选择截图，核对selected CG bounds、独立像素范围、实际内容/所有权与AX裁剪，同时确认Main取证不退化。真实截图、四游戏、sleep/wake与视觉门禁在新source seal同证书包、新nonce队列从头执行；原R59失败及38/50记录保留，不能拼接或继承。

## 登记合同、指纹与真实守卫

职责M15/C33，QA工具补充。schema_version=2须有非空canonical kebab additional_final_change_ids，去重且不包含原primary/任何历史phase ID。保留原final_change_id常量作为不可重命名的primary，但附加ID只从合同读取，不能从metadata或源码新硬编码取得。候选只登记verify-refactor-stability-r62。现无读取旧schema1的真实消费者需要；旧WI读取当前schema2即可保留原九义务，所以schema1作为当前reader格式明确拒绝，不隐式降级。

实际运行require_pin=True，合同条目必须恰一且SHA一致；追加未批准ID却无新Root独审pin必拒绝。批准依赖原精确authority流程，不加自报approved。新final WI可在本登记采用后再建立并冻结source-snapshot，避免future WI前像与本片采用循环。声明packaged runtime但ID未登记仅在development明确错误；未知普通change照真实diff、release照原全九义务，不据名字推断final，不改原release政策。

新增12个实际测试，原30 test方法与3helpers AST不变。新旧ID空diff必须同九义务/14links/cases/checks，缺runtime/组件不能通过，claimed coverage不能替代执行，非法/空/重复/phase碰撞/未知/未pin拒绝，重复CONTRACT pin两顺序和同值也拒绝。五源统一隔离42 lifecycle+48 runtime=90实际通过，原native27；仅工具纯测试，不是authority/正式包/游戏验收。
