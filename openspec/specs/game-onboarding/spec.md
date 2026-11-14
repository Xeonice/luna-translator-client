# game-onboarding

## Purpose

由 `docs/product/game-library-window-acceptance.csv` 生成的目标规范；实现与产品验收仍未完成。修改源文件后显式重新生成，不把生成成功当产品通过。

## Requirements

### Requirement: GL-01 首次启动
客户端 SHALL 空库页头可发现添加与运行目标连接；首版无任何游戏启动入口。

来源需求：`REQ-037`。

#### Scenario: GL-01-acceptance
- **WHEN** 首次启动
- **THEN** 空库页头可发现添加与运行目标连接；首版无任何游戏启动入口

### Requirement: GL-02 本地选择文件
客户端 SHALL 确认元数据前不保存不启动不注入。

来源需求：`REQ-037`。

#### Scenario: GL-02-acceptance
- **WHEN** 本地选择文件
- **THEN** 确认元数据前不保存不启动不注入

### Requirement: GL-03 拖入程序
客户端 SHALL 进入同一目标核对并可取消。

来源需求：`REQ-037`。

#### Scenario: GL-03-acceptance
- **WHEN** 拖入程序
- **THEN** 进入同一目标核对并可取消

### Requirement: GL-04 目录或多个候选
客户端 SHALL 明确选择实际EXE而非默认第一个。

来源需求：`REQ-037`。

#### Scenario: GL-04-acceptance
- **WHEN** 目录或多个候选
- **THEN** 明确选择实际EXE而非默认第一个

### Requirement: GL-05 快捷方式未解析
客户端 SHALL 要求核对实际目标不伪造路径。

来源需求：`REQ-037`。

#### Scenario: GL-05-acceptance
- **WHEN** 快捷方式未解析
- **THEN** 要求核对实际目标不伪造路径

### Requirement: GL-06 保存入库
客户端 SHALL 记录名称环境Bottle与目标路径；本机封面安全发现/持久引用可实际显示，失败占位；保存不启动。

来源需求：`REQ-037`。

#### Scenario: GL-06-acceptance
- **WHEN** 保存入库
- **THEN** 记录名称环境Bottle与目标路径；本机封面安全发现/持久引用可实际显示，失败占位；保存不启动

### Requirement: GL-07 重复目标
客户端 SHALL 同环境Bottle规范化路径匹配已有记录。

来源需求：`REQ-037`。

#### Scenario: GL-07-acceptance
- **WHEN** 重复目标
- **THEN** 同环境Bottle规范化路径匹配已有记录

### Requirement: GL-08 不同Bottle同路径
客户端 SHALL 保存为独立记录不合并专用身份。

来源需求：`REQ-037`。

#### Scenario: GL-08-acceptance
- **WHEN** 不同Bottle同路径
- **THEN** 保存为独立记录不合并专用身份

### Requirement: GL-09 编辑目标
客户端 SHALL 新身份不继承旧目标已验证状态。

来源需求：`REQ-037`。

#### Scenario: GL-09-acceptance
- **WHEN** 编辑目标
- **THEN** 新身份不继承旧目标已验证状态

### Requirement: GL-10 移出游戏库
客户端 SHALL 保留历史且不删除游戏文件。

来源需求：`REQ-037`。

#### Scenario: GL-10-acceptance
- **WHEN** 移出游戏库
- **THEN** 保留历史且不删除游戏文件

### Requirement: GL-11 选择运行窗口或进程
客户端 SHALL 同一选择面板核对有效目标，已有权限自动预览；仅一次确认关联即Hook，不另点核对或连接。

来源需求：`REQ-002`。

#### Scenario: GL-11-acceptance
- **WHEN** 选择运行窗口或进程
- **THEN** 同一选择面板核对有效目标，已有权限自动预览；仅一次确认关联即Hook，不另点核对或连接

### Requirement: GL-12 窗口跨环境映射
客户端 SHALL 分清Mac宿主与Bottle内Windows目标。

来源需求：`REQ-002`。

#### Scenario: GL-12-acceptance
- **WHEN** 窗口跨环境映射
- **THEN** 分清Mac宿主与Bottle内Windows目标

### Requirement: GL-13 映射不唯一或无映射
客户端 SHALL 要求重选或进程选择不能冒称成功。

来源需求：`REQ-002`。

#### Scenario: GL-13-acceptance
- **WHEN** 映射不唯一或无映射
- **THEN** 要求重选或进程选择不能冒称成功

### Requirement: GL-14 确认关联并接入
客户端 SHALL 一次确认自动启动Hook；正式关联无封面目标且录屏授权时保存首张有效帧，不保存候选/不覆盖；阶段区分且失败可重试。

来源需求：`REQ-002;REQ-004`。

#### Scenario: GL-14-acceptance
- **WHEN** 确认关联并接入
- **THEN** 一次确认自动启动Hook；正式关联无封面目标且录屏授权时保存首张有效帧，不保存候选/不覆盖；阶段区分且失败可重试

### Requirement: GL-15 重新关联与切换目标
客户端 SHALL 同完整运行身份复用且不重复注入，session预览跨页签常驻；不同目标一次确认切换，取消保留原会话/预览。

来源需求：`REQ-002;REQ-043`。

#### Scenario: GL-15-acceptance
- **WHEN** 重新关联与切换目标
- **THEN** 同完整运行身份复用且不重复注入，session预览跨页签常驻；不同目标一次确认切换，取消保留原会话/预览

### Requirement: GL-16 解除窗口关联
客户端 SHALL 保留Hook会话原句及历史。

来源需求：`REQ-043`。

#### Scenario: GL-16-acceptance
- **WHEN** 解除窗口关联
- **THEN** 保留Hook会话原句及历史

### Requirement: GL-17 自身保护与显式解除
客户端 SHALL 选中自身不接入；显式解除只清运行关联不清库记录。

来源需求：`REQ-043`。

#### Scenario: GL-17-acceptance
- **WHEN** 自身保护与显式解除
- **THEN** 选中自身不接入；显式解除只清运行关联不清库记录

### Requirement: GL-18 窗口退出或重启
客户端 SHALL 清旧关联并重新核对运行身份。

来源需求：`REQ-043`。

#### Scenario: GL-18-acceptance
- **WHEN** 窗口退出或重启
- **THEN** 清旧关联并重新核对运行身份

### Requirement: GL-19 取消或返回
客户端 SHALL 取消返回不改原库或会话，也不保存候选帧；全局连接始终可达，详情三重复按钮和所有启动入口移除。

来源需求：`REQ-002;REQ-037`。

#### Scenario: GL-19-acceptance
- **WHEN** 取消或返回
- **THEN** 取消返回不改原库或会话，也不保存候选帧；全局连接始终可达，详情三重复按钮和所有启动入口移除

### Requirement: GL-20 未知游戏或无文本
客户端 SHALL 未知或无文本不借用台词/封面；窗口关联不等取文成功，未运行游戏不由客户端启动。

来源需求：`REQ-002;REQ-043`。

#### Scenario: GL-20-acceptance
- **WHEN** 未知游戏或无文本
- **THEN** 未知或无文本不借用台词/封面；窗口关联不等取文成功，未运行游戏不由客户端启动

