# 设计与验证边界

继续维护 docs/design/first-product-v1/ 唯一整稿，使用现有材质、SVG、配对画面和设计数据。新增窗口预览、会话三页签和共享自定义Select，HUD演示拖动与页面内快捷键，分词/注音独立可观察。截图来源、回放和权限/失效状态明确区分。原型不读取真实系统窗口、不注册全局快捷键、不调用真实词法worker。

PRD详规与验收单一入口为 reading-window-session-prd.md / reading-window-session-acceptance.csv。原始PRD、License和旧97/153计划身份保留；新RW验收仅候选，未实施与产品执行均not_run。Root维护OpenSpec、影响/漏检文档和Playwright候选验证；PRD Owner维护需求；设计Owner维护整稿脚本；独立上下文审阅核对边界。

三类验证分别记录：设计交互检查；源范围/规范/编译/五负例工程检查；未来正式包原生/全量语义实测。后两者不能从设计截图自动提升。旧版15/44参考保持只读，新稿不自动覆盖旧参照；任何当前baseline drift须如实失败。

## Select间距反馈

通过明确行间隔及不可压缩选项尺寸隔离高亮；鼠标和键盘共用唯一活动项，已选值仅由勾记表达。禁用项不响应悬停，长名称自然换行。追加真实pointer hover、键盘与行几何检查，不仅拍打开菜单。
