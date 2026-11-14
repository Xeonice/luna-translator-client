# 原始桌面截图的完整窗口 referenceRegion 提案

只读分析；不修改原PNG、设计源、捕获脚本、守卫、阈值或批准状态。对照执行身份为 `.build/product-ui-candidates/2026-10-18T21-21-15-807Z/verification.json`，40行为通过、15视觉失败。本提案不将失败改成通过。

## 坐标如何取得

使用固定Playwright1.56.1/Chromium141，以原设计验证器相同的viewport1600×1100、deviceScaleFactor1打开本地整稿，只执行实际入口并读取DOM；不调用原截图脚本，不写批准目录。

- 原验证器 `scripts/report/verify_first_product_design.cjs:16` 固定viewport；`:28` 的capture默认截`#desktop`，因此PNG坐标必须扣除桌面BCR。
- 实测`#desktop` BCR `{x:0,y:58,width:1600,height:925}`，与原始PNG1600×925一致。下面的rect均为PNG相对坐标，已经减去pageY58；不能再加58。
- 场景38：`first-run`，与`verify_game_onboarding_cases.cjs:50..59`相同空库。
- 场景10：`reading`首句，点击`#hud-source [data-token-index]`中的風景；与原验证器点击流程相同。
- 场景11：接上场景10，点击`#review-next`，等待译文完成，再点`[data-learn-action="popover-grammar"]`。原句仍固定，HUD已到下一句。
- 元素身份在`docs/design/first-product-v1/index.html:7/8/10`。`styles.css`的原始几何：main left=50%-520、top85、1040×720；learning right25、top111、600×670。桌面1px上边框使PNG相对top分别为86和112。词卡由`learning.js:173..185`依据真实词语锚点定位，不能只凭静态CSS猜坐标。

## 可以注册的完整窗口区域

| 捕获ID / 原PNG | referenceRegion（整数像素） | 完整性 / 限制 |
|---|---|---|
| `learning-fixed` / `11-learning-unconfigured.png` | `{x:975,y:112,width:600,height:670}` | 完整学习窗，含标题栏、三色窗控、边框/圆角、原文、译文、新句提示、未配置卡与底栏；未被其他表面遮挡。可用于当前600×670候选。外阴影不属于DOM BCR，与原验证器直接locator截图的边界约定相同。 |
| `empty-library` / `38-library-empty.png` | `{x:280,y:86,width:1040,height:720}` | 完整主窗，但原PNG的等待HUD覆盖其下半部。区域必须保留这层遮挡，绝不能另裁洞、遮罩、跳过那部分像素。独立WindowRoot候选没有该HUD，故仍是场景差异。该crop只修比较画布，不证明空库已视觉通过。 |

冻结原PNG身份与只读内存像素实验：

| ID | 原PNG SHA256 | 完整候选 SHA256 | 区域RGBA SHA256 | 同尺寸pixelmatch结果 |
|---|---|---|---|---|
| empty-library | `3d7cf25d60565096a0dc9afbd5c7eab88f92f106d09f6eb9c3ebaddcc8fe1f09` | `504c5711f084160c208cbf35f950e4349029b5c1e276b9baf8c9a245f9bda41c` | `d3c88573ff69f96ae90a6bfdabe6a1aaf72bb58531bdbdb4924948fac744e59f` | 18,256像素 / 2.438034%，仍failed |
| learning-fixed | `ec9d8e7eb95b304f5d4ae41e76c5fd49b0b51246cd34a27eeece293b7a6f5546` | `dac2285bdfe08a16ad1682891263dc3d159b7b5a391af51b0686aa108587e338` | `5128942f43ce2060df659e398c5b2ba05faa6546f818eb79d9fdc39d379c0504` | 37,646像素 / 9.364677%，仍failed |

实验仅用pngjs读取原图、在内存提取上述完整整数rect，pixelmatch7.1.0 threshold0.1/includeAA=true/maxRatio0；未输出替换PNG、未缩放、未过滤文字或装饰。

等待HUD的实际DOM rect为`{x:320,y:611,width:560,height:125.78125}`，完全覆盖在主窗BCR内。若要验证未遮挡的单主窗，需要未来经评审的未遮挡参考或同态多窗捕获；不能通过crop删除这层真实产品表面。

## 不应直接注册为单WindowRoot相同场景的区域

### 10-local-word-card.png

实测HUD为`{x:320,y:451,width:560,height:146.5}`，词卡为`{x:375,y:187,width:300,height:268.4375}`（position:fixed）。词卡几乎全部在HUD外，只有约4.4px与HUD顶部重叠。

- 只裁HUD整数框`{320,451,560,147}`会删除正在验收的词卡，禁止用于`wordcard-native-size`或`dictionary-distinct-headwords`。
- 包含全部HUD和词卡的最小向外取整union为`{x:320,y:187,width:560,height:411}`。它包含两种表面和中间真实游戏背景，不是一个完整独立窗口；当前560×290/660×280候选与它几何、词语和词条数均不同。不得把union强制缩到候选大小。
- 可以将这个union作为未来同态多表面场景的参考，但前提是候选真实展示相同词面/词义及HUD，而非将参考词卡贴到产品上。当前两项为正确性fixture（放課後、箸/橋），没有对应批准图，宜保留reference unavailable/failed并另保留行为测试，不能用裁词头或删释义制造通过。

### learning-japanese-linebreak

原PNG同11，可追溯完整学习窗rect仍为975/112/600/670；但当前候选是1040×760的自造句及不同状态。单独加region不能合法消除尺寸/文本差异。原UI28的1040/560/390与18/36字体断言必须保留；如另做同态视觉截图，应单独走Sakura固定旧句流程或未来批准相应状态参考，不能给当前错误状态改名。

### 其余参考

add-game、target-review、library-batch原PNG已经是完整`#app-dialog`，分别760×966/760×880/760×1010；不得再裁正文或底栏去配688×648/574的产品表单。settings31、reading09、preferences32、model22、connection23、history20原PNG也已是对应完整窗口，referenceRegion应为null，不可再裁掉导航、标题或较宽字段。glossary当前对比model22，根本没有同态批准术语页参考；crop不能修正这个状态关联。

## 同尺寸仍存在的真实差异

- reading-hud，560×147：11.9060%。同首句原文/译文、注音、控制行和字号已经一致；仍有源/译文间距、译文baseline/行高、控制行高度与标签、背景材质场景差异。参考09在游戏画面上，本次测试harness背后是独立缩放的桌面壁纸，模糊背景不同；这部分需要同态背景捕获，不宜通过改token迁就。
- display-preferences，560×344：14.2644%。现在实际HUD大字+菜单；与参考32的菜单内容/锚点、字幕可滚动区域、字号后的密度和减少透明度表面仍不同。不能裁掉菜单或正文只比较底栏。
- settings31，1000×720：7.2447%；model22：7.6597%；connection23：7.6276%；history20，1040×720：5.5135%。导航/字段结构、部分实体样本、原生窗控与布局仍需真实对齐。模型/连接样本的实体名称和认证方式也非全部同态，不能全部归为颜色问题。
- 完整学习窗ROI仍9.3647%：生产译文仍随阅读偏好默认24逻辑px而设计16px；来源行/阅读记录标签、提示容器、底部模型来源与正文留白不同；浏览器候选没有真正原生窗控。应逐项评审偏好作用范围与真实来源数据，不能伪造记录序号或模型来源来匹配样例。

## 若root采用referenceRegion，建议保持的可执行契约

1. 仅对固定ID注册固定rect、原PNG路径/SHA256/全图尺寸、窗口selector和上述场景证据；不接受调用方在运行时自行挑区域。
2. rect必须整数、正尺寸、在全图内；候选完整窗口尺寸必须与region严格一致，不缩放、不挪齐、不二次裁候选。必须包含标题栏/边框/正文/底栏；缺失任一区域应拒绝注册。
3. 回执保留完整原PNG的只读副本与SHA、region、提取区域的RGBA或确定性PNG摘要、完整候选SHA、差异图SHA与source identity。独立adapter重新读取原PNG按冻结rect提取并复算，不信任runner写出的crop图。
4. threshold0.1/includeAA=true/maxRatio0保持；无论crop后差异变小多少，非零仍failed。像38的遮挡与10的跨窗口词卡要有显式场景限制，不能绕过。
5. 原有UI01..40行为及required six绑定保留；地区契约只修正比较画布身份，不把原生材质、焦点、真实Hook/词典/模型升级为通过。

## root收敛结论

- 未遮挡的学习窗可按完整600×670窗口区域比较，仍包含所有不一致区域并保持零阈值。
- 空库主窗的HUD遮挡必须通过真实Main+HUD同态合成或原生双窗桌面截图保留；不能删HUD像素来获得未遮挡主窗参考。
- 词卡在参考中跨出HUD原生边界。当前产品是在同一HUD WebView内绘制Popover，无法绘制到原生窗口外，这是产品架构差异。应另行设计独立native浮层/窗口及其固定对象、锚点、焦点和生命周期；不能用union裁图或贴背景假装已有同态浮层。union仅描述原参考的表面范围，不能作为当前单WindowRoot通过依据。
- 本批生产与测试已冻结。后续Model默认模板DTO与fixture适配由root/配置Owner接手，必须以新源码重新编译和执行，不能沿用这份旧源回执宣称新改动通过。
