# 当前完整设计

**目标ID：guided-service-reading-20261030；完整组合：complete-guided-history-20261031；完整源版与C35组件补充均已获用户确认。** [完整批准记录](../../development/design-approval-complete-20261031.json)绑定原guided57源与R79历史4源，共61源；默认入口仍为guided，历史入口由该组合显式覆盖。完整61源继续沿用父批准；C35运行窗口预览以[五项组件补充稿](candidates/preview-contract-20261104/index.html)为新设计目标，[组件批准记录](../../development/design-approval-preview-20261104.json)精确绑定补充的5个冻结文件与用户原话“采用这五项增量，继续更新”。原61源和补充5文件中的pending_review捕获标签均保持原字节，有效批准由各自独立决定解析。补充入口是C35原稿/拟稿与状态的对照画布，不是其他完整页面的现行入口，也没有生成新的完整Main组合。

客户端容器采用用户确认的[C1适配合同](client-layout-20261105/README.md)：Main／历史1040×720保留原双列及190px导航；设置采用A1000×720；760×560时使用紧凑导航，预览顺延到正文下方。该[独立批准记录](../../development/design-approval-client-layout-20261105.json)只批准客户端容器及窄窗适配，原61源与C35五项补充保持各自身份；设置B1040×760未被选用。逐态参照、生产组件及正式包仍须重新验证。

- [打开已确认的通用设置结构](candidates/general-settings-structure-20261107/index.html) · [读取/保存状态及Native参照环境](candidates/general-state-evidence-20261108/index.html)
- [打开已确认的三处布局修订](candidates/layout-refinements-20261107/index.html)
- [打开当前会话与阅读](candidates/guided-service-reading-20261030/index.html?scenario=reading)
- [打开当前HUD间距修订稿](candidates/hud-header-spacing-20261107/index.html?scenario=reading)
- [打开翻译服务配置](candidates/guided-service-reading-20261030/index.html?scenario=settings)
- [打开C35运行窗口预览补充与逐项对照](candidates/preview-contract-20261104/index.html)
- [全部页面、弹层与状态清单](../../development/all-page-design-coverage.md)
- [实现差距与后续任务](../../development/current-client-alignment-plan.md)
- [设计对齐及准出机制](../../development/design-alignment-and-release.md)

当前句在首屏独立呈现原文、译文、进行中/失败状态及就近操作；历史、全部提取和Hook诊断后置默认折叠，数量/警告/缺口仍可见，展开不丢各自筛选/页/跟随或重启预览。配置从用途、品牌和明确接口进入，只展示必要字段；学习与高级设置按需展开。日常切换服务使用统一浅层选择器，未配置不可选，保存/测试不切当前服务。

[三处局部修订](candidates/layout-refinements-20261107/index.html)已由用户确认“目前没有问题了，开始做更新修改吧”，精确9源及manifest由[独立批准记录](../../development/design-approval-layout-refinements-20261107.json)绑定：历史筛选/正文同轨与16px内容线、通用设置安全边距、停止会话旧句与重连层级及有效预览独立性。原稿中的pending标签保持冻结捕获字节。历史与停止会话继续按该补充实施；通用设置由[新结构批准](../../development/design-approval-general-settings-20261107.json)替代其组织与间距，保留五个原始捕获叶与两条依赖URL迁移记录，新入口[三组结构稿](candidates/general-settings-structure-20261107/index.html)维持既有字段继承、导航和系统权威。源稿原pending标签保留，批准由独立记录解析；[状态补充批准](../../development/design-approval-general-states-20261108.json)固定读取中（首次/刷新）、读取失败、保存失败，并另行确认Native透明展示底/系统窗口按钮环境。75份补充原件按原字节迁入[补充入口](candidates/general-state-evidence-20261108/index.html)，不改V3、材质token或父61/C1/C35。独立参照、真实组件与签名包Native分别核验，源确认不授产品准出。

HUD顶部服务选择器按用户要求增加上下留白：顶部栏38px、上下6px，选择器26px、右侧按钮24px保持原尺寸。[间距修订记录](../../development/hud-header-spacing-request-20261107.json)绑定原61源和仅追加一层CSS的组合入口；这是本次待核对的局部修订，不重写父批准或提升独立参照/产品准出状态。原61源、C1、材质与已批准图保持原字节。

视觉沿用已确认的连续暗蓝灰磨砂材质、中性线性图标和克制配色；专用Select、轻返回入口、HUD半透明与真实ruby/POS效果按状态校验。基准在[材料tokens](material-tokens.css)，不是靠提高透明度或忽略原生层替代。完整组合的manifest及61源闭包由[当前视觉合同](../../development/current-design-alignment.json)固定；C35五项增量另按组件批准记录绑定。补充将演示标签移到产品外，窗口身份只陈述已核实事实，保持紧凑卡片并由详情承载完整信息，分开有效画面与最近成功历史时间，并区分投影权限和核验能力。

## 当前历史页 · 已纳入完整源版批准

[打开历史页新稿](candidates/history-session-paging-20261031/index.html?scenario=history) · [浅色长文](candidates/history-session-paging-20261031/index.html?scenario=history&historyState=long&theme=light) · [加载失败与重试](candidates/history-session-paging-20261031/index.html?scenario=history&historyState=records-error)

历史页补齐按会话筛选、两条独立分页，以及加载/失败/重试/无记录/结束状态；“加载更早会话”采用R79轻文字操作，包含hover、focus、disabled、spinner、error与窄窗/200%约束。该精确源版已与guided共同获批；冻结pending_review标签仅保留捕获时事实。具体规则见[专项PRD](../../product/history-session-paging-prd.md)，客户端技术合同与CP-11已同步，设计回放和正式产品验收仍分别记录。

## 确认和验收的区别

原20261019的16源与37PNG确认保持原身份；[原批准记录](../../development/design-approval-20261019.json)不批准当前guided增量。旧v3/v4视觉套件继续验证其历史目标，不能满足当前产品改动的截图要求。HTML原型截图只证明设计演示；真实React状态、独立确认参考、同包原生窗口和游戏回归分别留证。

新参照须先独立生成、按页面/状态审阅并记录确认，再由生产组件执行器捕获候选和比较。禁止实现与参照共用截图、自动重拍批准图、放宽零差异阈值或用旧回执改标签。完整组合的页面状态矩阵覆盖当前阅读、三数据集、配置、服务选用、HUD/Select、主题/键盘/长文/窄窗/200%；参照或实际执行缺一即阻断。200%只获得焦点不等于整窗无裁切。

完整61源及C35五项组件补充的源版已确认；现有4项非C35独立参照继续按各自精确状态、源身份和[参照审核记录](../../development/current-reference-review.json)有效，不因本次组件确认扩展或失效。C35参照与产品映射仍需独立逐态补齐；其他缺失参照、真实React执行、正式包Native及四游戏回归分别验收，故“已对齐”及产品准出保持blocked。源版确认不代表当前809项适用状态（原802项完整保留）、正式包、Native、游戏或发布通过。详细检查与限制见设计对齐文档。原批准图、源码、共享资产、早期候选和历史报告均保留，但历史入口不再并列推荐。

[整理前文档快照](../../archive/documentation-r75/docs/design/first-product-v1/README.md)

## 通用历史行的明确语义替换

用户已选择[始终保存，改为保存状态与历史入口](../../development/settings-history-policy-20261104.json)。该决定替换通用页原型里可关闭的历史checkbox语义，不批准新的完整截图或改变其余61源。客户端采用同一FormRow/状态/轻操作规范实现；新的独立视觉参照与实际候选仍需分别核对，旧参照与失败保留，不据该决定放宽像素阈值。
