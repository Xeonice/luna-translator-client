# 历史页设计增量验证计划

依据[专项PRD](../product/history-session-paging-prd.md)与[待审完整候选](../design/first-product-v1/candidates/history-session-paging-20261031/index.html?scenario=history)。仅验证设计演示；当前正式包、Rust/Repository、原生、真实模型与游戏结果不继承。原FP/MC/RW、153及576集合全部保留。

| ID | 验证什么、怎样验证 | 产品状态 |
|---|---|---|
| HP-01 | **延续父完整稿和默认历史页**：默认入口、壳/侧栏/标题尺寸及材质继承；游戏/会话均为现有专用Select；增量待审不继承父批准 | not_run |
| HP-02 | **游戏与会话范围**：从真实可见Select及键盘选游戏/手动输入/全部，切会话范围正确；标签以时间、游戏及状态可读区分，切游戏清会话 | not_run |
| HP-03 | **加载更早记录**：首批50，追加至100/末批，真实DOM记录ID去重且已有前缀保留、当前选择和列表位置保留，结束不可继续 | not_run |
| HP-04 | **加载更早会话**：独立会话目录首批50、后续目录追加及结束；不替换原正文记录页、选中会话或重置另一个分页 | not_run |
| HP-05 | **记录加载和失败恢复**：首次加载/追加加载各有状态；失败保留已读内容/选择/编辑及位置，重试同一正确范围；跨范围失败明确旧内容所属范围 | not_run |
| HP-06 | **会话目录加载与请求失效**：加载/失败/重试/无会话各独立，切游戏快速重选时旧timer不覆盖新目录或抹掉新held回放 | not_run |
| HP-07 | **游戏推进与历史隔离**：真实滚动后父完整render及推进台词均保留筛选/页/选中/滚动；已打开词卡固定旧词，主游戏新句不抢历史 | not_run |
| HP-08 | **编辑事务与焦点**：编辑中禁用筛选/选择/分页/刷新/定位；草稿及caret跨父render保留，保存失败可恢复，取消直接还原退出无额外确认 | not_run |
| HP-09 | **当前句定位与译文迟到**：明确定位重置全部游戏/会话并固定正确当前句；原文先到与译文稍后状态可辨，迟到只回写所属原句不抢其他记录；当前句位于首批之外时，定位后继续加载至末页，完整ID集合无遗漏无重复 | not_run |
| HP-10 | **设计状态直达与恢复**：initial/records-loading/records-error/records-end/sessions-loading/sessions-error/sessions-empty/empty/long九种深链及完成回放；清楚仅评审工具 | not_run |
| HP-11 | **视觉与局部滚动**：深浅色、长会话名/长路径/日文长行、窄窗和真实200%布局缩放；选择器菜单/按钮完整可达，仅记录列表和详情各自滚动 | not_run |
| HP-12 | **证据及产品边界**：真实Chrome零pageerror/外网请求，回执nonce/源码/原父资源/截图身份及正断言零skip；设计通过不登记正式客户端/原生/游戏通过 | not_run |

## 前置、维护与回执

UI Owner维护新候选4源及显式pending manifest；QA维护本页/JSON、history_design.py、对应守卫与真实Playwright runner。需要本机固定Chrome、既有playwright-core及父设计本地资源，不需要真实账户、API Key、CrossOver或游戏运行。离线样本是设计fixture，不能当作产品数据。

每轮输出到新的`.build/<run_id>`：实际命令/退出码、nonce、父批准与候选源hash、实际断言、每项PNG hash/尺寸、失败与重跑关系。截图不覆盖原批准图片、不放宽零差异阈值；父壳/材质继承与新增历史内容评审分别核对。200%使用真实布局缩放，DPR2截图不能替代。

出现缺图、错源/旧nonce、零断言/skip、非法路径、虚构命令或产品通过声明须真实拒绝。无执行not_run、失败failed；用户尚未审阅候选时保持pending_review，设计自测不等于用户确认。

```sh
python3 scripts/quality/history_design.py --check --json
python3 scripts/quality/history_design.py --execute --output .build/history-review-run --json
npm run verify -- --change design-history-session-pagination
```
