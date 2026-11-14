# R30：已审生成worker的产品源码闭包指纹同步

状态：编码前治理卡；本轮没有产品或守卫代码修改，候选/独审/采用与新包实测分别留证。

| 项目 | 范围 |
| --- | --- |
| 授权 / Owner | Root明确授权修复单生成worker闭包；GovOwner `/root/r27_governance`，独立审阅 `/root/stable_signing_review`，采用由Root授权。 |
| 基线 / 来源 | R29原733项，SHA `032ece60b1e04ee27026c2d44a91abd80e06b3bf53f3b3b096fb86a8ed6ca012`；已审生产后全源 `be14fa5f7ecc23571f67fbb20c46363e97bd2b4f0e5a7310f64e11ad8315a579`。 |
| 依据 / 验收 | M15、REQ-056、RW-33/34；C34为最终打包窗口验收对象；既有整稿main-window/hud-window不变，无视觉/业务变更。 |
| 唯一指纹改动 | `implementation-status.json.product_sources_sha256[apps/desktop/src-tauri/resources/language-worker]`从旧2642175f…同步为生产生成并独立核对的新1eda050b…；其他447键值及全部模块/组件/能力/历史/实机状态完整保留。 |
| 允许治理路径 | 本change最小proposal/design/tasks/spec/verification、本卡与精确审记录；上述status单key候选；原render若确有变化的必要派生（否则不写）；R29原字节归档和R30基线候选。不得改guard、三签名实现、worker源/二进制、UI/Core/数据库、技术锁、债务或批准设计。 |
| 前镜像 | `.build/next-batch-20261020/generated-worker-closure-r30/before-authority/`及source-before.json；R29基线按原733项字节归档。 |

此前生成worker补充审真实核过生产/JA和全源单文件变化，但只核直接baseline项，没有覆盖status中的间接产品源码闭包pin。完整canonical08实际在implementation-status-contract拒绝旧指纹，Root已中断自有验证并保留partial failed；不能据生产审或08签名/ZIP/JA局部通过称完整工程通过。原09b、be14与08包/失败都保留。

本轮同步是已审生产生成物的精确身份登记，不放宽闭包、baseline或守卫。原40个场景及checks原值保留；新场景显式映射既有implementation-status-contract及原检查。独立核对单key、447值、其余全catalog值、原render、精确737项候选、源逆换与四消费者。无变化MD/CSV不采纳。

完成须经独审通过、Root允许实际目标后采用，并另建09新包与全量verify。08实际包源码身份已旧，只保留其局部证据，不能重贴新源标签。Luna首次权限、更新保留与四游戏仍独立实测；不据本治理修复提升TCC/实机/发布。不提交、不推送。
