# HUD 按真实翻译投影显示空态

## Why
R67 HUD中TranslationStack只按snapshot unit.translations空显示通用“尚无译文”，TranslationTaskStates同时用ReadingUnitProjection显示真实“翻译中”，两者并列误导用户。已有唯一query owner应统一决定HUD译文内容与空态。

## What Changes
HUD opt-in复用TranslationTaskStates的唯一useReadingUnitProjection，组合TranslationStack与已有TranslationStatusContent。默认Main/history/study调用行为保持。真实queued/running/partial/failed/cancelled/unordered/unavailable由已有任务行表达，不同时显示通用占位。初读继续R66 layout-stable status槽。unrequested/无engine只显示已知事实；不从空数组推断未配置。已知当前会话关闭自动翻译时按权威session状态显示。

## Impact
仅ReadingPanel.tsx、HudWindow.tsx及既有verify-reviewed-reading.mjs；既有100ID和RBC15/R66原断言完整保留。M03/M13/M15、C17/C18。无Rust/协议/hook/native/DB/资源/依赖/字体/行高/PNG/token变更。M03触发storage_requirement_gaps，开始即声明注册schema2 rust-storage-complete-v2，并必须实际执行。全部写入隔离R68目录；Main、运行游戏和已安装包不触碰。共享authority、baseline pin不自行刷新，完整verify可能因尚未采用而拒绝；Native与四游戏均not_run。
