# 迭代卡：已评审阅读与会话修订实施

## 授权及基线
用户2026-10-20明确授权下一批执行与四游戏回归。依据FP01..12、RW01..34、R20（431项，c7c8e877ca119d3949b9cec12b27c7bf7395f0f5b40d122f74ea3a5c692010df）、implementation-status审阅；原source539af6c0a37dfda864ab6d3c27fa714e12c45b7189637baf4e45ffb41fe79f70。当前无Git，记录source hash不虚构commit，不提交推送。

| 工作项 | 需求/验收 | 设计入口/状态 | M/C/Owner | 允许路径与复用 | 验证/完成条件 |
|---|---|---|---|---|---|
| RB-01 运行预览 | FP03/05 RW01..06 | window-preview 正常/权限/失效/取消/切换 | M02/12/13 C13/14/35 native+UI | platform/window_preview与C35；完整TargetIdentity及服务租约，可信实例映射 | 最终包2次可识别变化、权限/最小化/退出/错目标，原生与Hook分离 |
| RB-02 HUD平台 | FP04/05/11 RW07..12/24..25 | hud-controls/reading 工具/隐藏/冲突/回退 | M11/12/13 C01/18/34 native+UI | 复用window_manager/menus/偏好；原子几何文件、唯一shortcut coordinator | 500/560pt真实拖动/游戏前台快捷键/补偿/恢复/背景采样；无第二屏不冒充硬件通过 |
| RB-03 Select | FP11 RW13..16/31..34 | select-showcase 全状态/2px/单活动项 | M13 C04 UI | 已装ReactAria+自有CSS；所有有限选择入口 | 同态零差异/真实键鼠/长文200%/边缘内部滚动；原稿不覆盖 |
| RB-04 会话数据 | FP09/12 RW17..23 | session-history/hook-events 三页/跟随/筛选/导出失败 | M03/10/13/14 C23/36 backend+UI | 复用raw/单writer/Repository；002候选先评审/备份/回退 | 多页/选前/未选/暂停/切流/重启/GC/磁盘失败；真实producer逐序列比对 |
| RB-05 词法 | FP04/07/11 RW26..30 | ruby-material 八组合/错误/迟到/200% | M07/11/13 C16 UI+root | 既有worker/资源/Span校验，不增浏览器分词 | 实际随包worker到Rust到HUD/复制重构/旧结果隔离；历史语言质量失败不默认修好 |
| RB-06 闭环 | FP全部适用 RW31..34 | 原已确认材质/图标/组件全入口 | M15 QA/root | 原参考/v3留存，新v4独立审阅；构建独立candidate不覆dist | 新源码/同一最终包SR MO Never7 Sakura逐款完成，不测Ever17；静态/组件/原生/游戏各自回执 |

## 职责
UI worker独占TSX/CSS与UI tests；native worker独占platform和window_capabilities新DTO/已审桥接构建；backend worker独占session_records DTO、ports/runtime/SQLite及相关测试。Root维护contracts/mod/exporter、IPC/bootstrap/main、ApplicationPort/generated、包/注册/测试adapter/规范基线。交界变更先协商；并行不覆盖他人。独立reviewer/上下文审查分开实施者。

## 前置和边界
批准设计16源/37图、原PRD/License/token/001和旧制品有420项保护快照(.build/next-batch-20261020/context)。用户现授权优先于旧文档阶段说明。schema2/内部DTO/原生桥接与资源政策先形成可评审候选再独审，不盲刷旧contract/hash。测试原CSV保持定义；运行结果独立.build。现旧v3有源漂移，补批准源同态参考后才称产品视觉通过；91真实层/原153执行组合不能由开发绿灯替代。外部模型沿既有本机/内网服务，密钥不输出；桌面/Steam/权限若缺先记录并请求所需输入，独立工作继续。

## 完成与证据
新adapter应真实启动对应层，绑定nonce/source/package/资源/断言/截图；保护/架构/五类负例、storage适用真实adapter、实际编译均需通过。正式候选身份与实测记录置.build/next-batch-20261020，最终状态清单只据源码/当前实测分别标记；不build后改被封装source伪称同包。
