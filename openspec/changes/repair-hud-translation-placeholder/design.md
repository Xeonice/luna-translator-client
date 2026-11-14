# 唯一状态投影所有者内组合

1. TranslationStack增加可选emptyFallback，默认旧文案保留。
2. TranslationTaskStates增加仅HUD opt-in的translationContent({fontPt,compact})，复用现有唯一hook；根据Rust projection和严格匹配的当前session状态组合Stack与Status，不复制订阅或业务任务选择规则。
3. HUD替换当前独立Stack+States为同一个组合；现有queryFeedback=layout-stable持续保留。
4. 有真实projection时译文使用projection.unit，但snapshot在同一对象较新时不得回退旧译文；错误/刷新保留已知内容并标示既有上次读取。只有无译文、无任何可显示任务/错误且已读取时显示中性可恢复空态。
5. queued/running显示原真实任务标签及取消；partial继续readingPartialText严格task/attempt拼接；completed已有译文显示真实结果；失败/取消/invalidated/unavailable/unordered保留原行。unrequested如实显示“尚未发起翻译”；无engine如实显示“当前原句暂无翻译任务”，设置入口走现有ApplicationPort.openWindow。当前session明确translationEnabled=false才显示自动翻译关闭，不能假称未配置。
6. query初读、后台刷新只有持久可访问状态槽；实际错误Notice/retry可见，不固定HUD高度，不加入dummy翻译，不改stream/final排版。

FP-04/05/06/12，当前完整稿index.html#main-content、interaction-review.html#25-toolbar-560，设计原批准身份保留；此占位缺陷属于真实进度呈现修复，候选未评审，不提升产品/Native验收。

测试扩既有RBC38，保存原完整100ID与全部旧断言；先真实未修复Red，再完整注册adapter：真实Rust/SQLite fixtures→production React/hook/CSS浏览器。 held initial/loading、queued/running/partial/completed、failed/cancelled/unrequested/empty engine、query error/retry、旧对象迟到、single-flight、560/500与R66稳高均保留或扩充。批准图/零阈值保持，新增候选截图独立输出。

## 编码前Root语义审阅（2026-10-29）

Root同意唯一owner组合与默认caller保持，并要求明确竞态：view仅当commitRevision >= snapshot.commitRevision才供译文Stack；较旧as-of必须保留props unit，不能按translations.length猜freshness。hook身份验证、commit floor与100ms单飞完全不改。测试须覆盖snapshot先完成/held旧query仍空，以及query先完成/snapshot仍空两方向。空态确立后refreshing沿用同一as-of内容，禁止因无关commit增删占位制造fit脉冲；初读仅R66持久可访问槽。translationContent字体/compact仅用于Stack，partial样式原字节保持。当前游戏Auto继续，禁止采用/打包/退出。

## 首轮真实候选失败后的平手修订（编码前记录）

candidate-green01完整100中，原RBC09/17/61在同commit的新snapshot内容被等commit旧query覆盖而失败；不改原测试。AppSnapshot另有运行期revision，且现有调用者直接消费当前unit，commit平手没有理由让query覆盖props。因此精确策略改为：query commit严格大于snapshot commit才接管；等commit及更旧保留props unit。仍不按译文长度猜freshness，不改hook/commit floor；同identity由原hook完整校验。这是保留已有阅读内容契约的修复，不通过删断言规避回归。

新增RBC38另有夹具注入问题：query先完成时其as-of commit高于snapshot1，随后注入error只追平已应用commit，按原hook合同应不新查。修正受控输入为明确高于applied的commit，并让snapshot携带该真实已完成unit，再断言错误可见、完成译文保留、retry可用。新增同commit snapshot更新的正向竞态断言。原失败回执和candidate01完整map保持。

## candidate02 独审阻断与编码前修正设计（2026-10-29）

独审 reviews/candidate02-review.md 有两项实质阻断，原100/2356和storage通过不能关闭：

1. HUD外层hasTranslation仍只读props snapshot，query严格更新先完成时，“仅译文”模式会同时显示译文、原文与“尚无完整译文”提示，未形成同owner闭包。
2. 新retry夹具未同步到当前floor，原断言只检查特定注入错误消失，可能变成过时回执错误而假恢复。原hook无错误，不可改hook或弱化原断言。

Root已同意最小纯组合回调：translationContent增加可选render(contentUnit, translationNode)，由唯一hook owner选定同一contentUnit，再普通函数调用render返回ReactNode；禁止把新callback当组件类型渲染。HUD把原SourceText→translation→SourceTextStatus→仅译文无结果提示片段原样移为renderReadingContent，hasTranslation与fallback读取同一contentUnit。SourceText/词法/词卡仍使用原props unit闭包，DOM顺序与字号/行高均保留。三模式始终同一个TranslationTaskStates节点；source模式仅active=false且不挂translationNode，保持原本不查询翻译语义。无新state/effect反向同步、第二hook、镜像业务状态或Native变更，active=false不承担稳高修复。

新增真实生产组件断言：仅译文下query-first/snapshot-first/equal三向竞态正确隐藏原文及无结果提示；无完整译文时source↔both↔translation切换保留可见SourceText及词卡锚点节点和固定词卡对象，source-only无查询（真正完成的translation-only仍按原产品行为隐藏原文）；retry fixture满足当前floor，实际发出并完成新query，所有query读取错误消失且译文保留。全部旧100ID/原断言逐字保留，R66持久status槽在同模式后台查询期间保持。候选需新回执及Root独审，不继承candidate02为完成。
