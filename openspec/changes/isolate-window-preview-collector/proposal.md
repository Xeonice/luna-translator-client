# 独立窗口采集进程与纯订阅边界

## Why
用户于2026-10-24指出窗口采集应是独立进程，其他模块应只订阅。candidate16真实存在画面/过期/无可核验窗口周期切换：ReadingPanel绑定query reject清空binding，卸载订阅；后台主进程预览和窗口证明共享同步资源。R38只修部分清理路径，不能宣称本缺陷已解决。

## What Changes
先执行A：同一签名Luna可执行文件在最早入口分派专用worker，真实第二PID只拥有AppKit RunLoop、系统singleWindow picker/原filter、精确只读Windows与Native核验、有界采集和私有IPC。主进程的PoC监督者仅消费结果。无新框架/依赖、独立bundle或权限重置；现有Core/词法/DB不进入worker。同exe/证书/DR不等于权限已继承，必须实测。
A成功后才进入B：父PreviewService改为代理与latest cache，主窗/关联面板/封面只订阅，主App不再周期主动probe；明确控制命令与订阅生命周期分离。B源码暂未授权，须先补精确路径/设计/测试并按既有迭代规则登记。

## Verification
A只证明真实签名程序的双PID、worker自身TCC/picker/帧、父轮询干扰与订阅背压隔离。协议/取消/身份错乱/异常退出/资源边界有真实编译与测试。A绿不代表产品界面或最终四游戏通过；B后才进行同包真实变化画面/Hook/词法/封面回归。原候选14/15/16失败、视觉源漂移及8MiB未实现缺口保留。不提交或推送。
