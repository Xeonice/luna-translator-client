# HUD query feedback 的最小呈现修复

公开 TranslationTaskStates 新增可选 `queryFeedback`，默认 `notice`；HUD 唯一调用处选择 `layout-stable`。私有 TranslationStatusContent 保持唯一查询状态/任务呈现逻辑。瞬态 query 消息仍保留原文字并以 持续挂载具名 role=status / aria-live=polite / aria-atomic 提供可访问信息，使用局部标准 visually-hidden 内联样式脱离自然高度。实际错误仍为可见 Notice，重试按钮不变。各引擎 unavailable/unordered、真实running/queued/partial/failed/cancelled及取消不变。

不将 active=false 作为修复；useReadingUnitProjection owner、commit floor、single-flight、100ms合并、query invalidation不变。稳定状态布局与现有批准参考相同，HUD主文自然内容测量和native几何不变。

依据：FP-04/05/12；当前唯一整稿 docs/design/first-product-v1/index.html?scenario=reading 与 interaction-review.html#25-toolbar-560；20261019整稿及材质确认保留。本次是修正后台读取提示导致的异常尺寸脉冲，新增呈现语义待独立代码/可访问性审阅；不提升原生或正式产品验收。

完整before树先于源码修改冻结，候选与截图分开。新增RBC-15时序回归必须在未修复树上实际失败，修复树上实际通过；现有长→短、同内容去重、限高和owner late等断言全部保留。

## 修改前独立审阅与同根因前缀

Root已明确同意同两源纳入refreshing-only前缀。独立审阅hud_jitter_review指出：taskRow在后台刷新增删“上次读取 · ”也能换行。before生产HudWindow/hook/CSS于Chromium141.0.7390.37在560/500宽、running/failed四种长标签真实复现：task行分别增加13/15px，原译未变，完整fit时序在外部prefix-original-reproduction/observations.json。

因此HUD仅refreshing且无query error时，该stale前缀也采用同款不占布局的可访问span，DOM原文字保持，以保留RBC38旧断言和旧状态语义；实际query error时前缀仍可见。默认Main/history/learning样式与Notice语义保持。本次不合并流式换行/partial-final排版差，真实内容成长/注音变化继续自然适高。

独审建议采用持续挂载具名polite/atomic status槽，避免aria-busy=true抑制消息且卸载来不及播报。测试验证可访问树及槽持有，不宣称macOS VoiceOver或屏幕阅读器播报已验证。
