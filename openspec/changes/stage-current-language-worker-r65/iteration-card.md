# R65派生资源迭代卡

| 字段 | 内容 |
|---|---|
| identity | stage-current-language-worker-r65 / FP1-20261028-R64 SHA 5e29853a84589b0a51a331fd40aad009b80bdd592567b9fbb33187196b1e2cbb |
| 真实before | Main1950 tree 2e2652808333a4075f4ccbd2bcea36da97d53a6985bd555f8e44ac4c827c941f；old18b原字节已保存，Root恢复回执与build失败保留 |
| Owner / 独审 | /root/r64_batch_governance；source+gov及后续authority独审另安排 |
| 授权 | 只写language-worker-staging-r65 ignored目录；Main/系统/GUI/旧packet/adopt/提交禁止 |
| 范围 | exact apps/desktop/src-tauri/resources/language-worker + 自有7WI；公共业务/protocol不变 |
| 需求 / 验收 | FP-04/07/08/11/12；RW-26/27/29；LWS-001/002/003 |
| 设计 | 现行index.html#main-content；C16 SourceText/RubyToken 与 C19 WordCard；原图/token/零阈值不改 |
| 模块 / Owner | M07语言JA、M13 UI、M15 QA；仅派生资源，不跨业务实现 |
| 输入 | Root preserved actual2f=target current code；原build-source-map1950与当前before一致 |
| 检查 | full product-infra-tests（真MeCab hang/kill/newPID/restart）、强制v4、core/native/reading及标准身份/边界/编译/五负例；运行另记录，不预填通过 |
| 依赖 | source+gov独审 → Root精确集成 → R65 authority候选生成/独审/adopt → final63独审rebase/新freeze/新包 |
| 完成条件 | 所有适用真实回执可追，任何失败保留；binary smoke不替代registered、包或四游戏 |
| 当前产品实测 | 无新包/app；最终9/14/50与四游戏not_run；R64视觉/lifecycle失败不回填 |

## 本次可避免的时序问题与以后顺序

build-rc01在最后打包准备阶段才首次将当前app-core依赖对应的language-worker重编并stage到源资源，导致已冻结source与catalog仍是old18b而实际当前构建产物变成new2f。门禁正确拒绝这一个资源漂移，没有生成包/app。派生资源同步遗漏应在final freeze之前处理；Root精确恢复old18b只为保存失败并建立真实新change前像，不表示旧worker可作为current发布。

后续依次执行：业务或依赖变更 → 先编译并stage所有受影响派生资源 → 审阅实际构建provenance、精确hash及资源闭包 → 采用source authority后冻结最终cohort → 最后打包并继续严格拒绝任何源/资源漂移。不得把freeze移到prepare之后掩盖变化，不排除资源、不刷新旧hash迁就构建，不改guard或使用旧worker冒充current。

本片只在自有WI记录原因和顺序；共享治理文档与build工具不改。source候选的实际检查回执绑定该候选自己的完整source tree；Root按独审governance-after覆盖自有WI后形成的新Main/cohort必须fresh检查，不能把隔离候选回执改写为Root或最终包通过。当前R64实际Main1943工程64/66、旧final1950无package的事实及失败日志保持。
