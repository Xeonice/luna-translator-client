# サクラノ詩 10th Anniversary edition 实机取文验证

验证日期：2026-10-18。结论：**本机 CrossOver 中的 x64 游戏已连接 Luna Reader 0.1.1，推进对白后，原文能在实际客户端窗口中持续更新。** 本次验证了 5 次不同对白；两个候选文本流各产生 5 条非空事件，共 10 条，不能把总事件数理解成 10 句不同对白。

可视化报告：[离线 HTML 报告](../../reports/sakura10th-validation-20261018.html)。应用包：[Luna Reader 0.1.1 / macOS arm64](../../dist/Luna-Reader-0.1.1-macos-arm64.zip)。

## 1. 导入对象与运行环境

| 项目 | 本轮实际对象 |
| --- | --- |
| 游戏 | サクラノ詩 10th Anniversary edition；窗口标题显示 Re:Graphic 10th Anniversary edition / Ver1.0.0 |
| 来源目录 | `/Users/tanghehui/Downloads/archive/サクラノ詩 10th Anniversary edition` |
| 新建 CrossOver bottle | `Sakura-no-Uta-10th`，模板 `win11_64` |
| CrossOver | `26.3.0.39832` |
| 导入位置 | `C:\Games\SakuraNoUta10th` |
| 主程序 | `sakuranouta10th.exe`，x64 |
| 游戏引擎 | Artemis / Rev3257；实际 Hook 日志确认匹配 Artemis 引擎 |
| 本次连接身份 | Windows PID `204`，creation FILETIME `134367381726189540` |
| 身份复核路径 | `C:\Games\SakuraNoUta10th\sakuranouta10th.exe` |
| 游戏 EXE SHA-256 | `7c604c92b434cbee3620dcf4af8dde40d9313d04cefa794d168b03477135a08a` |

游戏从已有目录复制到新 bottle，未接管之前保留的另一台 Steam 会话。原目录、导入文件清单和主程序哈希见 [import-manifest.json](../../diagnostics/sakura10th-live-20261018-r1/import-manifest.json)；连接前的真实进程身份见 [target-probe.json](../../diagnostics/sakura10th-live-20261018-r1/capture-r1/target-probe.json)。这里的哈希标识主 EXE，不表示已为全部资源归档计算哈希。

## 2. 启动调整与客户端变更

首次在英文环境启动时，在尚未注入 Hook 的阶段已观察到白屏和标题中的 `????`。随后仅对本轮新建 bottle 设置 `LANG`、`LC_ALL`、`LC_CTYPE` 为 `ja_JP.UTF-8`，重新启动后，日文标题、主菜单及剧情画面正常显示。该先后关系支持继续使用此日文环境，尚不能单凭一次复测认定白屏的唯一根因，也不能把注入前的异常归因于 LunaHook。

本次保持原版 C++ LunaHook / LunaHost 不变，原生版本仍固定在 v12.1.2.3，commit `c8701872a5de83f8d3272f3967457d0b8f2983c4`。客户端补充了通用进程发现：通过只读 probe 的 `--all` 枚举目标，而非为 `sakuranouta10th.exe` 新增专用名称白名单。连接时继续复核 PID、完整路径和创建时间。

```text
CrossOver 中的 x64 游戏
  → 原版 LunaHook / LunaHost
  → Windows Rust 宿主的回调与 JSONL
  → macOS Rust 后台
  → 已打包的 Luna Reader 窗口：当前原文、候选流、历史
```

本轮使用 release 0.1.1 的真实 `.app`，不是控制台输出或浏览器中的示例页面。原 0.1.0 ZIP 保留。新包及宿主的真实哈希记录在 [build-manifest.json](../../dist/build-manifest.json)：

- ZIP SHA-256：`186210c2dca12dd668e883c55e4dc2b558ece82918f9dd704c1a9ce3eeff82cc`
- Rust 宿主 SHA-256：`c821d84902b1a8450d400fb538a8f3859ad54cf2f82174e93e34b58227abde61`

## 3. 可视化取文结果

客户端选择 bottle `Sakura-no-Uta-10th`，刷新进程，选择 `sakuranouta10th.exe · PID 204 · x64` 并连接。宿主原始记录依次出现 `ready`、连接接受、`process_connected`、Artemis 引擎匹配、`hook_inserted`、`stream_created` 和实际 `text` 事件。注入辅助程序的退出值不单独作为成功依据；本轮有后续回调、原始文本与可见窗口的对应证据。

两个候选流为：

| Hook | 完整流标识 | 非空事件 / 不同值 | 本轮观察 |
| --- | --- | --- | --- |
| **Artemis（选用）** | `204:5370399856:0:0` | 5 / 5 | 第 3 条保留「薄紅」，选作当前正文来源 |
| Artemis64x | `204:5370398544:0:0` | 5 / 5 | 第 3 条相同位置缺少「薄紅」，不选作本轮正文来源 |

选用的 Artemis 流实际收到：

1. `その風景には、悲しさも、痛みも、なかった。`
2. `だから、俺は泣かなかったのだろうか？`
3. `風に舞う薄紅は、たしかに美しくはあったが、俺にはただそれだけの事としか感じられない。`
4. `鼻につくような桜の香り――いや、桜にそんな強い芳香などない――の中を人々の手によって大事に運ばれる棺。`
5. `桜の美しさの中にいる親父の死骸は、ただ、醜いだけだった。`

每次推进剧情后都检查了实际游戏与客户端窗口。以下截图按同一条对白配对；第 3、4 条较长，游戏逐字显示时抓取的画面可能尚未全部展开，不能据此宣称已测量逐字渲染时延。

| 对白样本 | 游戏窗口 | 客户端窗口 |
| --- | --- | --- |
| 1 | [20-game-menu](../../diagnostics/sakura10th-live-20261018-r1/20-game-menu.ui.png) | [24-first-live](../../diagnostics/sakura10th-live-20261018-r1/24-first-live.ui.png) |
| 2 | [27-game-line2](../../diagnostics/sakura10th-live-20261018-r1/27-game-line2.ui.png) | [28-client-line2](../../diagnostics/sakura10th-live-20261018-r1/28-client-line2.ui.png) |
| 3 | [31-game-line3](../../diagnostics/sakura10th-live-20261018-r1/31-game-line3.ui.png) | [32-client-line3](../../diagnostics/sakura10th-live-20261018-r1/32-client-line3.ui.png) |
| 4 | [35-game-line4](../../diagnostics/sakura10th-live-20261018-r1/35-game-line4.ui.png) | [36-client-line4](../../diagnostics/sakura10th-live-20261018-r1/36-client-line4.ui.png) |
| 5（完整显示） | [43-game-full-line5](../../diagnostics/sakura10th-live-20261018-r1/43-game-full-line5.ui.png) | [44-client-full-line5](../../diagnostics/sakura10th-live-20261018-r1/44-client-full-line5.ui.png) |

第 5 条等待游戏完整显示后再次截图。游戏因布局换行，客户端因窗口宽度换行；文字内容一致。客户端同时显示已接收 10 条总事件、Artemis 当前流 5 条历史，证明正文来自实时会话而非一张静态占位图。

## 4. 正常断开与日志完整性

点击客户端“断开”后，记录显示宿主接受 `shutdown`，两个流均 `stream_destroyed`，随后 `process_disconnected`。最终快照为：

```json
{
  "status": "exited",
  "host_running": false,
  "text_events": 10,
  "errors": [],
  "integrity_lost": false,
  "shutdown_acknowledged": true,
  "exit_code": 0
}
```

原始 stdout 共 144 条 JSONL 记录，stderr 为 0 字节。正常断开与宿主退出已验证；这些证据没有独立证明 DLL 在所有线程状态下都能安全卸载。

- [原始 stdout.jsonl](../../diagnostics/sakura10th-live-20261018-r1/capture-r1/stdout.jsonl)：实际回调事件及文本。
- [requests.jsonl](../../diagnostics/sakura10th-live-20261018-r1/capture-r1/requests.jsonl)：身份约束连接与退出请求。
- [final-snapshot.json](../../diagnostics/sakura10th-live-20261018-r1/capture-r1/final-snapshot.json)：最终 UI 后台快照及历史。
- [stderr.txt](../../diagnostics/sakura10th-live-20261018-r1/capture-r1/stderr.txt)：原始 stderr，0 字节。

## 5. 本机启动入口与后续预览

本机已新增可双击的启动器：[サクラノ詩 10th Anniversary edition.app](</Users/tanghehui/Applications/CrossOver/サクラノ詩 10th Anniversary edition.app>)。它启动本轮独立 bottle 中的游戏。已通过该入口完成一次冷启动，实际出现 Ver1.0.0 的游戏窗口，macOS 进程 PID 为 `93068`；见 [冷启动截图](../../diagnostics/sakura10th-live-20261018-r1/56-launcher-cold-start.ui.png)和 [UI 现场](../../diagnostics/sakura10th-live-20261018-r1/56-launcher-cold-start.ui.json)。

CrossOver 当前侧栏缓存尚未刷新；独立 bottle 的存在和运行已通过实际启动确认。用户可以直接使用上述本地启动器。

冷启动后进行了独立重连，留给用户继续观察。用户手动进入对白后，[74-restored-client.ui.json](../../diagnostics/sakura10th-live-20261018-r1/74-restored-client.ui.json) 中实际可见客户端状态为“正在接收原文”、Artemis 已选中，当前原文为 `その風景には、悲しさも、痛みも、なかった。`。这属于新的预览会话，与第 3、4 节的受控 `capture-r1` 分开记录，不追加到其 5 次对白、10 条事件统计。

最终预览又保存了[游戏当前截图](../../diagnostics/sakura10th-live-20261018-r1/76-preview-game.ui.png)与[客户端当前截图](../../diagnostics/sakura10th-live-20261018-r1/77-final-client.ui.png)，两边均为上述同句正文，客户端在前台且 Artemis 已选中。**本轮收尾时保留当前连接，供用户继续推进台词观察。**

[最终验证摘要](../../diagnostics/sakura10th-live-20261018-r1/verified-result.json)分别记录受控验收与运行中的预览；[证据哈希清单](../../diagnostics/sakura10th-live-20261018-r1/evidence-sha256.json)用于对应已保存文件。`live-preview-log-snapshot/` 是运行中的日志副本，本轮未对这个预览请求退出，也没有退出后的 `final-snapshot`。其 5 条文本事件包含配置测试符号，不能称为 5 句对白。

## 6. 结论范围与复现步骤

本轮通过的是这款游戏在本机新建 CrossOver bottle 中的 **x64 原文实时接收与实际可视化展示小样本**。未验证在线翻译、长时间稳定性、全流程/全部分支、全部游戏兼容性，亦未进行端到端延时基准测试。

1. 双击本机上述游戏启动器；也可在 CrossOver 的 `Sakura-no-Uta-10th` bottle 中运行 `C:\Games\SakuraNoUta10th\sakuranouta10th.exe`。
2. 启动 Luna Reader 0.1.1，在“兼容环境设置”中选择同名 bottle。
3. 刷新进程，选择本次实际运行的 `sakuranouta10th.exe`，点击“检查并连接”。PID 和流地址可能随重启变化，应以本次刷新结果为准。
4. 进入剧情，选择 **Artemis** 流，推进对白，观察当前原文与历史同步更新。
5. 使用“断开”结束本次取文会话。

本记录的受控验收统计来自已落盘的 `capture-r1`。启动器冷启动与后续预览在第 5 节单列，不合并到这 5 次对白与 10 条事件统计。
