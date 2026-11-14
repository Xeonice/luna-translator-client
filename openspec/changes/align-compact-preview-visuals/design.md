# 视觉与复用方案

权威为complete-guided-history-20261031已批准61源，重点review-gaps.js:57-101、review-gaps.css:39-85及guided/flat-layout.css最终级联。旧37图继续保留历史用途，不作为恢复旧Tab、开启/关闭preview或启动游戏按钮的理由。

C35用实际数据保留同一组件/订阅：346CSSpx卡、54标题、16:9画面、46动作栏，>=520CSSpx容器横排；图像width100%、min-width0及object-fit contain，背景/blur/saturation/glint沿冻结token，降低透明度/无backdrop能力有fallback。短说明用于日常，原始diagnostics和完整身份迁入现有React Aria/Sheet可访问详情；沿HistoryScreen现有navigator.clipboard模式复制，不编造API没有的Native事实，不复制帧payload。打开/关闭/复制/焦点与状态变化均不调用capture start/stop或替换producer。

复用Controls/Sheet仅增加显式preview appearance，default/library保持原类/DOM语义，不扩大共享样式影响。新增PreviewDetails真实弹层必须在实际AST全页闭包中登记并追加状态，不隐藏入口。阅读改动限定overview/独立折叠区的最终computedStyle，不全局改字体/SourceText/HUD或伪造服务picker。

视觉两端独立加载：批准设计源与真实React生产组件，固定相同frame/title/time/状态/viewport/scale/theme/font/preferences；只规范测试宿主容器，不patch产品选择器或截图图像。起始permission/closed不带旧帧，paused/stale可保留；不能为原型残留图绕过生产权限清帧。原图不覆盖、新ref单独候选+非作者审阅，零像素阈值不放宽；Native材质与全794+新增义务另验。

## 边界缺陷与本轮明确修正

实际PW证明批准原型在纵排容器接近520CSSpx前，100%宽16:9画面会超出346高并盖住摘要/动作（外宽519/520/521的内宽为517/518/519，尚未触发横排）。本轮按“信息/恢复操作不被遮挡”的既有产品约束，产品纵排图框以标准282窄卡的内部280px图框为上限并居中，>=520内容宽横排恢复原512px上限；不针对某个测试宽度特判，不改卡54/346/46和16:9，不复制原型缺陷。旧61源原字节与边界失败截图保留，边界像素仍按原型差异记录，不自动批准新参考或称全量对齐。

实际Vite压缩会合并顺序不当的backdrop声明；局部组件改为prefixed在前、标准属性在后，实际生产产物与computedStyle核对，无token或构建target/框架改动。复制pending需保持焦点且拒绝重复，详情fixed定位须换算同一zoom坐标，并独立验证200%视口可达。
