## ADDED Requirements

### Requirement: REQ-038 真实本机封面与正式关联首帧回退
系统 SHALL 优先读取已核对游戏的真实本机封面；无封面时仅在正式关联有效目标且录屏权限已具备后，把该目标首张有效帧保存为托管静态封面。系统 SHALL NOT 保存临时候选帧、持续录制、自动覆盖已有封面或把设计样图当实际游戏封面。

#### Scenario: real-cover-asset-roundtrip
- **WHEN** 游戏入库后发现真实本机封面，或无封面游戏正式关联有效窗口且获录屏权限并收到首张有效画面
- **THEN** 封面按核对过的游戏归属进入安全资产显示链并可重启恢复；不存在有效资源或保存失败则如实占位，Hook 不受影响；候选取消、旧generation和错绑窗口不生成封面

### Requirement: FP-03 单面板核对后一次关联并接入
系统 SHALL 在同一附属面板展示目标选择、完整身份核对与预览，一次关联确认后即启动 Hook 接入，随后等待候选并由用户显式选择正文。

#### Scenario: one-confirmation-association-and-hook
- **WHEN** 用户从全局连接入口选择或点选有效运行目标并在同一面板确认“关联并接入”
- **THEN** 不经第二确认即启动真实 Hook；同目标复用不重复注入，不同目标取消保留旧会话；无效身份拒绝，收到候选不等于正文已选

#### Scenario: natural-disconnect-cleans-only-owned-session
- **WHEN** 当前真实游戏进程自然退出，并在native清理中、完成后或迟到旧事件期间尝试接入另一个已运行目标
- **THEN** 本session退出由有界异步停止owned broker、等待producer/relay/EOF和FIFO drain后按expected SID与完整target compare-clear；consumer不等待自身barrier；清理中明确可重试，仍活动他目标保持显式切换；迟到旧断开/错误/诊断/封句只作用事件SID且不污染新session，不用latest或强制确认绕过

### Requirement: FP-02 首版只连接已运行游戏
系统 SHALL 只提供已运行游戏的连接入口，移除首版所有游戏启动用户入口以及详情三个冗余连接按钮，保留全局连接入口和必要会话恢复。broker/helper 的必要管理 SHALL 不被误删。

#### Scenario: running-game-only-entrypoints
- **WHEN** 用户浏览空库、游戏卡片、详情、菜单和当前会话
- **THEN** 全局连接运行游戏可达，详情不出现“运行并连接”“关联并接入”“连接已运行游戏”三个重复按钮，卡片操作和菜单不启动游戏；必要更换/解除/断开恢复仍可用

### Requirement: RW-01 已授权目标自动预览且关联后常驻
系统 SHALL 在选中有效目标且录屏权限具备时自动预览，正式关联后由会话持有预览并跨当前会话页签常驻，不要求每次手动开启。持续预览 SHALL 不保存帧；REQ-038 明确授权的单张封面例外保持独立。

可采集还 SHALL 以强身份归属可证为前提；录屏权限不证明native窗口归属。非Unicode HWND SHALL 在任何可变标题挑战写入前拒绝并按现有类型明确报告unavailable，不自动切换A/W接口或helper位数重试；这项受限能力 SHALL 不阻断已核验Windows进程的Hook取文，也不得误报原生窗口已绑定。

#### Scenario: automatic-session-owned-preview
- **WHEN** 用户选择有效目标或完成系统录屏授权，并随后关联和切换正文/全部提取/Hook日志页签
- **THEN** 无额外开启动作即显示正确目标连续真实帧；权限拒绝、撤销、最小化、退出、切换、取消及迟到帧有正确状态和时间，不阻断Hook或改变游戏尺寸焦点

#### Scenario: native-title-proof-fails-closed-before-write
- **WHEN** 运行目标的HWND不是Unicode窗口，或完整HWND/PID/路径/创建身份、唯一nonce、受限时间等证明条件不满足
- **THEN** 不写非Unicode窗口标题，不用相似标题/矩形/乱码别名或替换A/W接口判成功；现有类型明确报告不可用原因，Hook生命周期保持独立；安全Unicode路径仍须exact nonce与原始标题恢复均可核验
- **AND** proof及恢复独立于discovery的visible/title过滤，全程重核根HWND/PID/完整image/FILETIME/冻结architecture/class与严格caption；隐藏或最小化不令恢复绕过强身份，也不等于该状态可采预览

### Requirement: REQ-052 保真且可排障的 Hook 日志
系统 SHALL 沿既有窄 Core 链路保留有界脱敏的原生消息和可证结构事实，正确映射 Warning1/EmuWarning2，并默认抑制低级 text_unselected 刷屏。系统 SHALL 保留原始提取、事件顺序、warning/error与丢失事实的可追溯性。

#### Scenario: useful-native-diagnostics-without-raw-noise
- **WHEN** 接入产生 Ready、Log、Warning、HookInserted、InjectionFinished 或 Error，并伴随高频未选正文回调
- **THEN** UI能定位实际架构/资源、引擎/扫描/安装地址、错误码/退出码和来源事实而非仅通用文案；默认视图不被逐条raw处置淹没；完整分页搜索导出与全部提取保留，凭据/私人路径/控制字符/超长payload受限脱敏，未知事实不编造

#### Scenario: window-title-proof-errors-remain-explicit
- **WHEN** 可变标题证明在写前被拒绝，或Unicode挑战/恢复/子进程回执发生超时、错误或身份变化
- **THEN** 受限原始错误与失败阶段可诊断，不把退出零值或Drop尝试当恢复成功；维护的hook-client与target_probe实际测试命令必须非零执行，零测试、忽略/过滤、失败和源漂移不能生成通过回执；最终包实测另记不从单测继承

### Requirement: FP-09 会话历史固定范围的安全恢复
系统 SHALL 保持既有范围TTL、DB、cursor身份与公开契约，在客户端按typed错误区分单次跟随恢复与用户显式回到最新，不把一般失败或无效cursor当作自动重置许可。恢复 SHALL 保留旧数据直到新范围成功，并维持single-flight与session/dataset/filter/epoch隔离。

#### Scenario: following-typed-snapshot-expiry-recovers-once
- **WHEN** 当前有效请求在follow=true时返回typed snapshot_expired
- **THEN** 只发起一次cursor:null读取；成功才整体替换当前范围/行/cursor，失败保持旧行和可见错误并允许显式重试，不递归自动恢复、不改TTL或静默截断历史

#### Scenario: paused-or-invalid-range-requires-explicit-latest
- **WHEN** 用户暂停浏览时遇到snapshot_expired，或任意跟随状态遇到invalid_cursor（包括GC）或cursor_range_changed
- **THEN** 保留旧行与浏览锚点，不自动cursor:null或跳底；用户通过既有“回到最新”显式读取新范围，恢复失败不清掉旧内容，普通错误也不自动reset

#### Scenario: history-recovery-isolated-from-stale-and-general-errors
- **WHEN** 恢复期间session/dataset/filter切换、用户暂停、旧请求迟到、新读取失败或发生其它typed/一般错误
- **THEN** 旧对象响应不污染当前状态、用户暂停意图不被迟到恢复覆盖，维持single-flight；一般错误无自动重置，旧分页/失败/多页/迟到断言继续成立，历史查看不改变Hook/选流/正文归属
