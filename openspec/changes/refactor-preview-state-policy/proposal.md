# SR1 预览私有纯状态规则提取

用户已明确要求执行整个既定重构计划。本片先固定现有行为，再仅提取WindowPreviewService内已有的身份/帧接纳和状态投影规则，降低重复状态判断造成的交叉影响。没有新增用户能力。沿用R47的14条状态合同，公开API/DTO、Native、worker、Hook和所有预算不变。

剩余SR2～6另有精确子change；本片通过不等于正式包或稳定性阶段通过。原product-ui-capture-v4来源失败保留并独立处理。
