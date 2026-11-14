## ADDED Requirements

### Requirement: GVC-001
系统 SHALL 按已批准General V3对齐通用页及受影响设置壳，保留原业务和其他窗口默认。

#### Scenario: general-shell-and-material-match-approved-source
- GIVEN 已批准V3和真实WindowRoot/端口
- WHEN 显示General或导航十类设置
- THEN footer在nav外按flex贴底，按钮字体间距按源；General单层frame，其他frame默认不变
- AND 源迁移、独立参照、真实组件及Native分别验证，原失败/0阈值保留

### Requirement: GVC-002
系统 SHALL 由代码权威、独立审核参照和实际执行关闭本页视觉状态。

#### Scenario: independent-general-visual-state-closure
- GIVEN 精确V3身份、本页14状态及基础视口主题缩放矩阵
- WHEN 捕获参照和候选、同包Native并独立重算
- THEN 缺源/映射/参照审核/实际层或存在差异即拒绝；固定投影只证渲染，OS/DB实际行为另验
- AND 新宿主/adapter先准入独审，本页结果不改变R90其余义务和全局完成判定
