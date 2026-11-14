# 迭代卡：绑定工程影响登记 r2

| 字段 | 内容 |
|---|---|
| Change / kind | register-binding-impact-surface-mappings / tooling，同ID明确修订，不走legacy no-op |
| 原实际父 | R93/c6554836…/2812/739；ParentSeal94983be及36f760独审、原P1/P2/P3完整保留 |
| 已实施起点 | 4515c79a…/2821，a0bc/b11授权19QA/docs、d71独审；checkpoint71c5完整图和26源字节归档 |
| 失败事实 | PID37963 exit1，原validator拒四M15 anchors，完整回滚；8909/c98不是实际采用 |
| 当前许可 | 2733只读方案独审+Root366804仅7canonical发布；snapshot905794保持，实际6diff；其他Main写一律未授权 |
| 需求/场景 | 原BIM001～008/30场景保留，新增H09/H10/P03/P04，共34 |
| 精确范围 | 原29+catalog3=32；门禁阶段gate3；以后原子catalog3+metadata3=6；ComponentsCSV不变 |
| Gate Owner | /root/binding_resumption_gate_implementation，限phaseguard/testphaseguard/verify |
| SourceAudit Owner | /root/binding_source_audit，保留I1，未来只在ignored准备catalog3/QA-only proof |
| Storage Owner | /root/storage_integration_fix，原严格存储修复保留，本r2不另改storage |
| Root Owner | 精确Root5docs子集另授权；整体源审/新六输出实际事务与最后fresh验证 |
| 作者/审者 | 原4实际作者不变；非作者integration_plan_review不能成为实现作者 |
| snapshot | managed source-snapshot仍实际c655，不能把4515或任何after预测重拍为原Base |
| 新权威 | canonical-plan-review-r2/qa-authorization-r2精确新原件与code-owned pins，实际CLI+verify必须active r2且无r1 fallback |
| catalog边界 | 新active r2/v2六输出锚点在实际字节冻结后再审；单current QA-only proof，739只保R93；M13只next_step901→944 |
| 准出 | 原fullBIND/v5/五CLI/完整守卫/TS/Cargo真实执行；旧185不授新源产品成功；product incomplete/nextstagefalse |

本卡的条件是计划义务；未来review/auth路径不是当前已存在的签字。当前7源待独审，不授gate/catalog/metadata/Rootdocs写入。
