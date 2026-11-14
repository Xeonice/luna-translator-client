# SR6 已落工作项的 R65 rebase 迭代卡

Owner：Root负责R65资源/authority独审采用与完整工程检查、真实source_files前像采样、rebase精确采用、最终source/package/输入和真实客户端动作；QA独立复算，AP核对证据，用户协助工具不可达的正常操作。协作者共享工作区，不覆盖回退他人。本工作项已在R64真实落入Main；本次rebase准备包仅在ignored目录，尚未修改Main。

需求FP-12，来源FP-03/04/05/07/12；设计为当前整稿interaction-review.html#01-window-preview及既有阅读/词卡状态；模块M01/M02/M04/M07/M10/M12/M13/M14，组件C11/C13/C14/C18/C19/C34/C35/C36。verification.json的ID、source_snapshot引用、scope、requirements和全部checks逐字节不变；delta spec亦保持。Rust业务权威、窄Tauri、独立Core、语言worker与单writer边界不变。

只允许修改 `openspec/changes/verify-refactor-stability-r63/**` 中本次精确审阅的proposal/design/card/tasks及Root后续真实采样的source-snapshot。当前资源R65八inputs与authority由其原Owner和Root另行独审采用，不能借final工作项改worker/资源/工具/authority。此准备只写 `.build/stability-refactor-20261027/final-validation-r65-preparation/**`。

R64实际记录：batch已采用1943源／1552a2...，完整工程检查64/66失败为visual及其lifecycle传递；final63七WI落地后1950源／2e265...。build-rc01因当前worker将staged18b→2f而被preparation守卫拒绝；无包/app/runtime。新旧worker、旧七WI、1943snapshot、1950map、所有失败原件均备份，不覆盖。Root恢复old18b仅恢复该失败前像，不表示new worker已采用。

rebase前置：stage-current-language-worker-r65的实际exact packet/独审/Root adoption、实际R65baseline与资源2f闭包、完整verify真实回执与失败分类。本草案形成时这些未由准备者确认，不能填预测数量/hash；实际检查如仍含视觉/对应lifecycle失败，原样记录为failed而非全绿。任何额外错误先回Owner修复再冻源。

Root之后从完整已审R65Main采before（含已存在final63旧七WI、new2f、R65WI/authority），精确审阅后只采用本片自有五文件差异（四说明文稿+真实snapshot），再重新采最终build map。原verification与spec不变；全新final-stability-r65同源ZIP、安装、活动profile、权限/AX/OCR、四游戏、普通退出与真实SleepWake仍待实际供给。Sakura旧PID/启动时间不继承为新run；每步重采真实完整身份。

完成条件：原50步及10runtime case、9义务/14links/29+21维护链接、同source/ZIP的storage和独立复算、原wrapper gate均有实际证据；typed retention附加场景逐条列实际触发或not_run/blocked，不用generic50冒充。视觉14failed与compact仅设计/暂不实施保留，不能据此关闭整个重构或首版153组合/发布。不提交、不推送。

Root实际采用本rebase前，仅绑定已经真实发生的R65 adoption/fullverify与完整新before及其原始回执，修正文稿中的准备期状态；不要求尚不存在的最终build-map身份。文稿可预先引用稳定输出路径 `.build/stability-refactor-20261027/final-stability-r65/build-source-map.json` 和 `source-freeze-receipt.json`，不预填最终map/hash。四文稿与真实snapshot形成新exact packet经独审并采用五文件后，才从source_files采最终map；实际数量、tree/map SHA及独审冻结结果另存上述ignored外部回执。采用后不为填写最终map/hash回写这些源文稿，否则该源冻结失效，必须另开显式新轮。
