# 修复 HUD 后台查询刷新引起的高度脉冲

## Why
受控真实生产组件已经复现：同原文同译文下无关 commit 触发状态 query，临时 Notice 插入/移除使自然内容增加约62逻辑像素。R65 native高度记录为独立观察；未将每次原生高度变化归因于本提示块，最终同源包因果对齐尚未执行。输入诊断 manifest `ab9e37235fdbd4e4e37dd1004f59d575941b0139f5b58eb532cba7181ec13177` 独立冻结。

## What Changes
只为 HUD 显式启用 layout-stable query feedback。复用 TranslationTaskStates/TranslationStatusContent，loading/refreshing 采用脱离文档流但可访问的 status 文本，默认主窗/历史/学习继续现有 Notice。错误/unavailable/任务状态/partial/取消/重新读取均保留。

## Impact
产品仅 ReadingPanel.tsx、HudWindow.tsx；回归仅 verify-reviewed-reading.mjs，扩充既有RBC-15，保留全部100义务与原断言。不改hook/协议/Core/Rust/native autoFit、限高、手动resize优先、CSS/token/批准PNG/像素阈值。候选不安装、不操作运行客户端、不提交推送。词法及流式真实内容变化仍可改变高度，本次不声称全消除。共享authority需要Root另行独审采用；作者不刷新pin。
