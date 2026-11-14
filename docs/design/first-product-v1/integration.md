# 设计原型模块约定

仅供本高保真HTML的协作与维护，不是生产架构。用户已确认首个可用产品范围：游戏取文、翻译、内置分词/词典、按需AI精读、模型配置。M1技术版与此产品集合分开；不表示真实功能已经实现。

## 当前设计契约

窗口核对后以“关联并接入”一次确认启动 Hook，按[窗口 PRD](../../product/game-library-window-prd.md)维护。同目标复用活动会话，不同目标先确认切换，失败保留有效窗口并可复核重试，解除关联保留取文。关联、Hook 接入、收到回调与选定正文仍是独立状态。

本目录是离线高保真原型，不访问本机窗口、游戏、词典或模型。设计脚本的自动检查/进度是明确的演示状态；不能作为客户端研发或实机成功。产品源码与构建不属于本轮工作。

## 文件与所有权

- shell worker：`index.html`、`styles.css`、`app.js`，桌面/主窗/连接选流/游戏/HUD/历史/手动/诊断及窗口层级。
- settings worker：`settings.js`、`settings.css`，独立设置窗内部及连接/模型/用途实体数据。
- library worker：`game-library.js`、`game-library.css`，空库、程序入库/资料编辑、运行窗口/进程映射与关联并接入。
- review-gaps owner：`review-gaps.js`、`review-gaps.css`，共享预览renderer/state/actions、详情可访问层、会话三类数据与阅读交互；会话和选窗不得复制为两套预览。
- Root：`data.js`、`learning.js`、`learning.css`，演示数据与分词/词卡/独立精读；文档、覆盖、截图、打包。

脚本顺序由 `index.html` 维护：图标和数据先加载，共享控件、游戏库、设置、学习与 `review-gaps.js` 在 `app.js` 初始化前可用；都是普通script，支持file://。不使用fetch/真实服务，数据为明确标记的设计样本。

## 共享API（由app.js提供）

全局 `window.LunaDesign`，简称`app`：

```js
app.state = {
  selectedGame: null, // first-run empty; restored scenario selects an owned record
  page: 'library', // library / session / history / manual / diagnostics
  windows: { main: true, settings: false, learning: false },
  session: { gameId: null, status: 'idle', selectedStreams: [], paused: false, translationEnabled: true, generation: 1, integrityLost: false },
  current: null, // immutable unit snapshot, defined below
  history: [],
  settings: window.LunaSettings.initialState(),
  learning: window.LunaLearning.initialState()
};
app.render();
app.openWindow('settings'); app.closeWindow('settings');
app.openSettings('grammar'); // optional purpose; settings window over current reading
app.openLearning(unit, tokenIndex = null); // freeze a copy, do not change HUD
app.notify(message); // low impact transient message; persistent failures use in-place UI
app.escape(value); // HTML escaping helper
app.dialog({ title, description, body, confirmLabel, onConfirm, cancelLabel });
// body is HTML, onConfirm returning false keeps dialog open; fields remain queryable
app.closeDialog();
```

窗口容器：`#main-window`、`#settings-window`、`#learning-window`；设置内挂载`#settings-content`，精读内`#learning-content`。独立词卡`#word-popover`，HUD `#hud-window`，正文`#hud-source`。通用dialog为`#app-dialog`；紧凑预览使用同级共享详情dialog，不能覆盖正在选择窗口的`#app-dialog`，详情根也不能带`data-preview-id`而被预览重绘替换。各模块只能改自己窗口内容，不能互相hidden替换整场景。

## 插件接口

`window.LunaSettings`：

```js
initialState();
render(app); // returns HTML for #settings-content; outer native window stays in shell
effective(settingsState, purpose, gameId); // purpose: translation / grammar / word
// => {enabled, connectionEnabled, modelId, modelName, connectionName, outputMode,
//     historyCount, status, revision, source}; absent binding => enabled:false
setRoute(app, purpose); // select translation category & appropriate purpose; no auto-enable
```

设置模块委托事件用`data-settings-action`、字段`data-settings-field`，事件发生时从`window.LunaDesign`取API。独立连接、模型、用途必须存储为字典或列表，不复用单一endpoint/model/prompt；草稿与已应用版本分开。

`window.LunaLearning`：

```js
initialState(); render(app); // returns HTML for #learning-content
open(app, unit, tokenIndex); // freeze input, open independent learning window
sourceHTML(app, unit); // ruby/token DOM for HUD; plain source is in unit.source
```

Root词卡模块通过委托监听`[data-token-index]`，记录点击时unit克隆。主窗历史可用`app.openLearning(record)`；正文复制使用`unit.source`，不用ruby的textContent。

## 数据结构（data.js）

`window.LunaDesignData.games`数组：`id,title,subtitle,exe,arch,bottle,accent,cover,scene,language,hook,streams,lines`。

`lines`元素就是阅读unit：`id,gameId,source,translation,tokens,explanation`；token为`{surface,reading,lemma,pos,meaning}`，顺序完整拼回source，标点/空格也保留无reading的token。解释为自造学习样本，不冒充实机回调。

主app克隆unit并补 `sessionId,revision,time,translationStatus`，推进时替换current并保持history归属；精读只克隆，不指向current可变引用。

外部评审工具仅调外观与故障注入/游戏推进，不用‘成功状态’下拉替代用户主旅程。可给`?scenario=first-run|existing-library|reading|settings|learning|history|failure&theme=light|dark`作截图直达，实际主旅程仍需全部按钮实点。

`LunaLibrary.libraryGames()`只返回入库记录；`allGames()`含明确的既有素材目录，不能据此宣称本机已安装。`openAdd`、`openEdit`、`remove`管理记录；`openRunning(app,{gameId})`统一运行目标选择与关联并接入；选择窗口仅预览，确认后才触发接入协调。`getBinding`、`unbind`、`onConnected`、`onGameExit`只管理本次运行关联。活动会话使用冻结的`gameSnapshot`，编辑或移出库不得暗中替换取文目标。任意新程序没有对应素材时，`streams/lines`为空，不进入假成功阅读。


## 2026-10-19 六项缺口沿革与当前约定

本轮只修订离线高保真交互，不修改 `apps/`、`crates/`、真实窗口权限或原生服务。入口仍为 `index.html`；新增说明入口由 `interaction-review.html` 汇总。原 token、原截图与 acceptance-v3 均保持原位，不因候选修改重拍或批准。

- `controls.js` / `controls.css`：所有产品区域有限选项复用自定义 combobox/listbox。隐藏的原 `select` 只保留字段值与原事件契约，产品可见触发器为 `.luna-select-trigger[data-select-for="原字段id"]`。无ID字段由 `data-settings-field` / `data-gl-field` 生成稳定ID，点号转连字符。原生 `selectOption` 不能验证这层交互，QA应实点 trigger 和 `[role=option][data-value]`，或真实键盘方向/确认/取消。页外评审栏保留原生工具选择器。
- `review-gaps.js` / `review-gaps.css`：运行窗口预览、当前会话三类数据、快捷键草稿/应用与组件状态。`LunaReviewGaps.sessions` 按会话保存本次设计运行全部提取与日志，不用每流最新样本覆盖；每屏12条只是候选显示页，能加载此前全部事件。未选流和选流前事件分别保留。连续点击“推进台词”回放已配对样本，重复回放产生新的设计事件序号，不能当作新的实机采集。
- 按2026-10-21明确覆盖，有效目标且权限已具备时自动预览，正式关联后会话跨页签常驻，不再通过 `enable-preview` 手动开启。图片取自已有配对素材；`previews` 按宿主窗口ID隔离，暂停保留最后画面和时间。只有出现新配对画面才改变成功画面时间，同一图片的刷新不更新成功时间。未授权、不可用和退出由页外评审栏演示，不触发系统录屏或网络。正式实现需另行验证连续真实帧、权限和窗口身份，不能继承本稿结果。
- HUD 顶部 `.hud-drag-handle` 与底部空白区支持页面内拖动，按钮、可点词和正文滚动不作为拖动区。隐藏再打开保留本页位置，恢复按钮重新定位；不冒称已实现原生窗口移动、重启持久化或跨屏恢复。
- 快捷键编辑和当前生效项分开；“应用快捷键”校验修饰键、应用内重复和常用编辑组合。失败保留旧组合。设计响应使用 `KeyboardEvent.code` 核对字符，避免Option修饰后字符变化；输入框编辑不触发阅读动作。默认候选沿用原稿 `⌥⌘H` / `⌥⌘P`；显示模式未设置。系统占用与注册失败必须由正式原生实现回执证明。
- `learning.js` 允许关闭可点击分词而单独保留注音ruby；词性只在可点击分词开启时可见，未知读音不补造，词法故障先保留原文。所有token和读音仍为既有人工设计样本，非本地词法worker的实际输出。

设计场景：`?scenario=window-preview`、`session-texts`、`session-logs`、`components`、`hud-plain`、`hud-ruby`、`hud-reduced`、`hud-unknown`、`hud-lexical-error`。动态锚点分别为 `window-preview`、`session-history`、`hook-events`、`select-showcase`、`hud-controls`、`ruby-material`。已具备权限的有效目标自动显示预览；产品内字幕/窗口控件由真实页面点击使用，页外工具仅注入评审条件。

`RW-01～34` 均为待实施/待实测需求；浏览器设计QA不提升产品、原生或153个正式发布组合的状态。


### 本轮补充：浮窗常用操作前置

顶部轻工具区提供置顶、紧凑/舒适和隐藏，其余空白与手柄可拖动；底部32pt工具行直接提供分词、注音、词性、原译显示、自动翻译、暂停/继续、重译、复制与精读。图标提供统一tooltip和可访问名称，无原句时相应动作禁用。更多菜单只保留低频的设置、关联/解除、恢复位置与断开，不再重复常用动作或主窗历史入口。按钮均复用原状态和动作处理，无第二套业务逻辑。

暂停跟随以当时文本/日志的末端序号冻结可见窗口，新事件只累计提示；恢复跟随才移向最新。搜索和筛选覆盖全量集合并保留浏览窗口，Hook单独事件也累计新增数。空列表键盘确认不选中不存在的项，错误后成功重试会清除控件错误态；这些是候选交互QA的必要组合，不以单张截图或仅条数相等代替行为验证。

## 2026-10-27 紧凑预览整合

用户已确认采用[独立紧凑方案](../../../.build/stability-refactor-20261027/compact-preview-design-v1/index.html)，入口SHA为 `ed53777dbccf61d9e2ee584f823404ea0da337002874eb55db9720ad8636387c`。本轮只整合离线设计，不改产品客户端、原API、原批准PNG/catalogue/token或旧候选。

`LunaReviewGaps.previewHTML(app,target,{selection:false})` 是唯一预览renderer；当前会话与 `LunaLibrary.openRunning` 选窗面板均调用它，继续使用同一个 `previews` Map和原state/actions。保留 `enabled/paused/updated/scene/frameSource` 的含义、`#window-preview` 会话入口、`data-preview-id` 根、`data-preview-owner`、`data-preview-target`、`.rw-preview-meta`、刷新/暂停及权限恢复动作。不以第二份脚本或另一套状态实现紧凑UI。

主卡标准282×346、16:9原图contain，固定标题/状态、原时间/身份摘要与常用操作；260窄卡及720/960宽卡按组件实际宽度响应。完整稿大视口里的窄侧栏不能触发宽卡布局。长标题和诊断不缩字硬塞：完整字段移到可达详情，主卡不整体滚动，仅详情正文允许滚动。详情可复制全部，Tab/Shift+Tab循环、Escape关闭并回原对象按钮；keydown在详情层阻止传播，仍保留正常复制默认行为。

详情从所选对象、现有预览Map及明确标识的离线数据读取完整游戏名/窗口名、Bottle/EXE/路径、Windows及宿主启动身份、窗口身份、hash、原采集时间、阶段/API/raw原因和隐私；缺少事实显示未提供，不把固定候选样例冒充所有目标的真实原生信息。详情打开期间的配对帧推进和整窗render不能丢失详情或换错对象；取消候选仍保留原会话和预览，迟到旧帧不污染新候选。

自动/暂停/开启中/权限缺失/不可用/退出/身份异常及长名/长错/日语由设计条件演示。恢复入口只表达真实产品的下一步，不在离线稿申请权限、激活或改变游戏窗口、启动游戏、重注入Hook，也不因点击而伪造成功。既有“一次关联并接入”流程保持。

本轮截图与执行回执写入 `.build/stability-refactor-20261027/compact-design-integration/worker/`，汇总约定为 `final-tests.json`；运行成功与否以实际回执为准。原旅程和帧/时间/暂停、会话不变、取消身份隔离等业务断言保留，完整字段检查通过真实详情入口访问。现有434条历史manifest文件项与原图哈希不重建、不刷新；新整合身份单独增量记录。产品、原生及四游戏状态仍为 `NOT_RUN`。
