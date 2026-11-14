# 设计与迁移

权威设计见docs/development/gate-validation-design.html，用户批准指令见gate-design-approval.json。

## 统一执行
固定代码注册命令，工作项仅引用检查ID，不提供任意shell。实际diff和依赖决定必选检查；不扩大legacy-debt。真实命令记录输入、日志、时间、退出与断言，输入变化失效。G01/G06/G08/G09不能自填passed。快检不能授予交付。

## Hooks
SessionStart只读；PreToolUse明确越界拒绝；PostToolUse观察实际变更；Stop按实际交付核验，问答放行，外部/人工blocked不循环。全局hooks保留，真实事件和模拟分别判定，不能伪造信任。

## 人工视觉
G09独立消费唯一对照报告。当前完整可比且有授权明确确认才通过；未确认blocked、拒绝failed、未执行not_run，相关变化失效。隔离审核fixture不能冒充产品批准。

## 迁移
开发统一入口升级为新执行器，原verify.py和断言保持历史审计可用；现行规范覆盖更新，不复活旧审批循环。原完整产品、存储、真实服务和四游戏义务不减，不以工程通过冒充产品通过。

## 验证
32组正反、恢复、故障、变异分L1/L2/L3留原件；实际TS/Rust/测试编译及正式端口和存储执行单独记录。最终源冻结后重验受影响项并独审。
