# 设计原型模块约定

仅供本高保真HTML的协作与维护，不是生产架构。用户已确认首个可用产品范围：游戏取文、翻译、内置分词/词典、按需AI精读、模型配置。M1技术版与此产品集合分开；不表示真实功能已经实现。

## 当前设计契约

窗口核对后以“关联并接入”一次确认启动 Hook，按[窗口 PRD](../../product/game-library-window-prd.md)维护。同目标复用活动会话，不同目标先确认切换，失败保留有效窗口并可复核重试，解除关联保留取文。关联、Hook 接入、收到回调与选定正文仍是独立状态。

本目录是离线高保真原型，不访问本机窗口、游戏、词典或模型。设计脚本的自动检查/进度是明确的演示状态；不能作为客户端研发或实机成功。产品源码与构建不属于本轮工作。

## 文件与所有权

- shell worker：`index.html`、`styles.css`、`app.js`，桌面/主窗/连接选流/游戏/HUD/历史/手动/诊断及窗口层级。
- settings worker：`settings.js`、`settings.css`，独立设置窗内部及连接/模型/用途实体数据。
- library worker：`game-library.js`、`game-library.css`，空库、程序入库/资料编辑、运行窗口/进程映射与关联并接入。
- Root：`data.js`、`learning.js`、`learning.css`，演示数据与分词/词卡/独立精读；文档、覆盖、截图、打包。

脚本顺序 `icons.js`、`data.js`、`game-library.js`、`settings.js`、`learning.js`、`app.js`；都是普通script，支持file://。不使用fetch/真实服务，数据为明确标记的设计样本。

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

窗口容器：`#main-window`、`#settings-window`、`#learning-window`；设置内挂载`#settings-content`，精读内`#learning-content`。独立词卡`#word-popover`，HUD `#hud-window`，正文`#hud-source`。全局dialog`#app-dialog`。各模块只能改自己窗口内容，不能互相hidden替换整场景。

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
