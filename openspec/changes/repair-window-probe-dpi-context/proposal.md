# 修复窗口查询线程的 DPI 坐标口径

## Why
用户报告机器重启后进程列表有游戏而运行窗口列表没有，预览不可达。2026-10-26 同一游戏 Win272 / HWND0x3005c / Artemis 严格 ABA 只读诊断：默认查询线程 GetWindowRect=(35791390,20,1179,768) 倒置；仅 helper 当前线程 PMv2 后=(-4,25,1474,960) 有效；恢复原context后再次倒置。身份和恢复检查均成立。原矩形保护正确拒绝坏数据，不能以删保护或猜窗口代替修复。installed/candidate22/23 两架构探针字节一致，不能认定最近Native变更引入该探针变化；此前完整重启发现链路回归未覆盖。

## What Changes
仅 Windows helper 的窗口枚举和点选坐标调用在当前查询线程 PMv2 scope 中执行，保存并恢复原context。动态API缺失、设置或恢复失败明确报错；恢复前不输出成功JSON。保持身份、反向/空矩形、系统窗口过滤、Native配对和取帧安全门槛。负坐标在宽高有效时合法。不改变游戏窗口或游戏/系统DPI设置。

## Scope
生产唯一源 crates/lunahook-host/examples/target_probe.rs；测试复用同文件真实scope算法。资源仅由现有prepare_core_runtime维护入口双架构构建stage。Root负责OpenSpec、架构源码登记/精确评审基线、打包与GUI；HK Owner负责该源。无新框架/依赖/数据库/IPC/UI或产品行为。

## Verification
同文件scope正负例、双架构真实编译、同一窗口新旧实际probe对照、实际签名包的窗口列表/绑定/Native首帧及变化帧、Hook/词法分别记录；睡眠恢复独立实测。未执行not_run，条件缺失blocked，既有视觉/发布缺口不提升。不得提交推送，不覆盖旧包/失败证据或批准图。
