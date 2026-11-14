# 设计与责任

root拥有schema只读比较器、注册/基线/报告整合；field_boundary_audit拥有字段义务、合法fixtures与探针；storage_contract_explore拥有存储子验收登记及实际adapter执行门禁；storage_context保存上下文，最终独立复核工具。并行协作不回退他人改动。

schema-contract.json与文档DDL分离冻结，默认只检查不重写；比较真实sqlite_schema/PRAGMA，覆盖STRICT、列序/类型/NULL/default/PK、复合FK顺序/动作、CHECK、索引及触发器，格式差异有限规范化，不把相同列名当作结构一致。原设计DDL资源不作为生产migration入口。

每字段必须存在适用边界、声明指纹和实跑探针；原CREATE TABLE投影保留全部列/表CHECK、唯一性但隔离FK/trigger，明确仅字段层。合法before/after控制、精确错误类型/目标归因、SQLite合法转换必须实测；未冻结业务容量政策不得杜撰上限或把pending当通过。原37全DDL关系/业务测试保持独立。

生产门禁仅执行代码注册的compile/run argv，不执行JSON自报命令。缺adapter、未执行/skip、旧源码/包/日志、缺候选数据库、结构差异和缺子验收均拒绝；storage变更开发验收和发布必须调用实际adapter。当前缺正式实现，只完成门禁，真实产品保持blocked/not_run。

## 审查发现与授权范围内的修正

字段穷举边界发现原SQLite TEXT length/GLOB/json_valid在嵌入NUL时接受不规范后缀，违反已有canonical u64/hash/JSON全文语义。本轮在60处原CHECK追加instr(field,char(0))=0并重新生成对应字典、明确重冻结schema契约；字段数、关系和产品范围不变。此为用户要求边界验证所暴露的设计修复，新增细节仍待复核，不修改产品实现。独立oracle确保已知非法值不能因观察到SQLite接受而被登记为正例。
