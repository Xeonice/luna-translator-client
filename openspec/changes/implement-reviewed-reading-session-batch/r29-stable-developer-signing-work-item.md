# R29：稳定开发签名与真实包身份

状态：编码前工作卡；实现、独审、最终包与权限回归分别记录，当前不预填通过。

| 项目 | 规定 |
| --- | --- |
| 用户授权 | 用户已创建证书并要求后续打包使用；修复现有打包脚本最终封装被强制覆盖为ad-hoc的缺陷。未授权提交或推送。 |
| Parent | `FP1-20261020-R28`，729项，baseline SHA `4f80cbadbf2b6aa1d4d256a6320038de92a48dd1ff08f2691e70b028f76398e5`；完整源 `f9ef8bc87535bfa750800d9b4d57686f7f57cd7d2bf0c99c5c4a0cae330f7c1b`。 |
| 工作项 / 依据 | R29；M15 / REQ-056（维护者检查、构建及来源）、FP-12、RW-33/34（同一最终包真实证据及变更闭环）。 |
| 设计 / 组件 | 既有整稿 `index.html#main-window`、`#hud-window`；C34为被打包的原生独立窗口验收对象，本轮不改任何UI、窗口行为或批准设计。 |
| Owner | `/root`负责打包修复，可将下列三文件整组派给唯一实现Worker；`/root/r27_governance`仅负责本change规范/归档/基线候选。实现与精确身份由独立人员审阅。 |
| 实现允许路径 | **仅** `scripts/build_desktop.py`、`scripts/quality/package_identity.py`、`scripts/quality/test_package_identity.py`。不得顺带修改quality注册器、其他checker、产品Rust/UI/Core、数据库、依赖、技术锁或批准图。 |
| 治理允许路径 | 本change的proposal/design/tasks/spec/verification、本卡与精确审记录；R28原字节归档及经独审的R29基线候选。GovOwner另负责仅在候选中精确更新implementation-status对上述三文件的当前源码引用，先保存全部旧引用清单，再绑定稳定且独审的新hash/真实引文；448产品哈希、其他引用、历史与状态不改。仅采用原render确有确定性变化的MD/module CSV，不复制无变化派生文件。实际采用另需Root允许。 |
| 前镜像 | `.build/next-batch-20261020/stable-signing-r29/before.json`、`source_before.json`、`before-product/`三原件及`before-authority/`；R28归档候选置于同目录`candidates/`，实际尚未采用。 |

## 最小行为合同

1. `--signing-identity`与`--ad-hoc`互斥。开发签名选择顺序为**显式CLI > 非空`APPLE_SIGNING_IDENTITY` > 唯一valid codesigning identity自动发现**；显式输入必须确实解析到可用身份，不因失败再尝试低优先级来源。缺失、歧义、无效或签名失败均明确失败，不默默降级。ad-hoc只能用显式`--ad-hoc`，不能靠默认值或把`-`当开发身份绕过。
2. 同一个已解析leaf SHA1身份传给Tauri构建和写入源码seal后的最终应用签名，保留现有entitlements及签名flags，但不得保留旧ad-hoc requirements。实际codesign失败即退出；不关闭验证、不覆盖系统trust设置、不在产品中写证书指纹或名称特判。
3. 签名后**实际读取**mode、Identifier、TeamIdentifier、Authority链、CDHash、designated requirement与leaf证书SHA1；开发签名必须与选择到的证书一致。源码seal、Mach-O、EXE/ZIP及资源身份仍由现有机制检查，不以env或manifest标签代表真实签名。提取输出应留在stdout或`.build`，不能再次误向仓库根写`:-`等产物。
4. 新seal以`signature_contract`、`signing_mode`、`expected_leaf_sha1`绑定合同，签名后取得的`signature_metadata`同时记录到ZIP的`luna-build-identity.json`和外部`build-manifest.json`；消费者解包后实际codesign严格验证，并核对两份metadata与真实包，拒绝缺失/部分字段、相互不一致、ad-hoc冒充开发签名、证书/DR/CDHash不符及伪造外部标签。
5. 原schema1历史包在**没有新增签名metadata**时仍走原有严格源码seal/资源/EXE/ZIP/codesign检查，不补造开发身份；一旦任一新metadata出现即走完整新合同，不能删除部分字段退回旧分支。原有签名无效、改包、跨源、路径和ZIP安全断言保留，不弱化现有测试。此marker只防删除/修改外部metadata造成的新旧分支降级；不声称防止攻击者有能力重签整个.app并重写seal的分发认证攻击，仍是既有local build provenance边界。

## 前提与证据边界

Root已修复本机证书中间链且核到一个valid identity；两份不同Mach-O的原生探针实际签名成功，DR相同并双向满足对方DR，见`.build/signing-identity-diagnosis/native-signing-probe/cross-requirement-result.json`。这些只证明本机签名能力及探针要求，不证明Luna的新包、首次系统授权、更新后权限保留或游戏通过；原失败回执保留。

当前07程序保持不动。首次开发签名Luna仍需用户在系统设置主动授权；不得替用户改TCC数据库、重置授权或把证书链有效直接记作权限保留通过。

## 验证与完成条件

- 参数/发现的真实决策分支：CLI覆盖环境、环境覆盖自动发现、唯一/零/多个valid身份、非法显式输入、显式ad-hoc与冲突、签名命令失败且无fallback。通过现有`test_package_identity.PackageIdentityTests`加入测试，不增加新注册类/脚本。
- 消费者正反例：新开发/显式ad-hoc实际metadata一致、模式/Identifier/Team/Authority/CDHash/DR/leaf不符拒绝；旧schema1无新增字段仍严格检查；原全部断言保留。既有注册ID为`package-identity-tests`，canonical原入口执行，守卫测试不代表真实包通过。
- 实际新包：独立候选目录构建，保留原dist/07与旧失败；实际codesign、seal/源/EXE/ZIP/两manifest身份由原消费者核验；构建失败不能留下被接受的成功manifest。
- 实际权限：用户首次授权后，核对真实Luna窗口帧；正常退出/重启及同证书另一个新构建的权限情况单独实测。第二包CDHash/EXE可以变化，实际DR/证书身份稳定须单独核；不从两探针继承结果。
- 采用前独审本三文件与本change增量、精确身份及R28原字节归档；数据库/旧债务/技术锁/批准设计pins不得刷新。原38个scenario及其16项checks保留，新场景追加映射既有`package-identity-tests`。
- 当前重矩阵、新Luna包、TCC/更新保留和SR/MO/Never7/サクラノ詩10th同包回归均`not_run`；外部条件缺失用`blocked`。R28/V3原文与四游戏义务不因签名修复取消；Ever17排除。
