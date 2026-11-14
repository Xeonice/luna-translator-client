# 现行入库与窗口核对设计的实现对齐

- work_item: IMP-LIBRARY-CAPTURE-ALIGNMENT-20261019；parent IMP-ACCEPTANCE-CLOSURE-20261019，R13依据不变。
- 授权：用户要求两项验收规划并真实通过；修复现行FP-02/03及GL-02/04/06/07/08/12的既有实现缺口，不新增批量编辑能力。
- Owner：library_capture_alignment（后端/契约），visual_acceptance（UI）；独立审阅：另行code/architecture reviewer。
- 设计：当前整稿40-add-game-review、42-window-mapping、46-custom-batch-review；M01/M02/M10/M12/M13，C12/C13/C14。

## 公共契约影响与决定

1. 新增 SaveGames 批次新增命令，items={itemId,game:GameInput}；批次1..100项（有界IPC资源策略），必须带本轮 expectedRevision。所有项在同一 Immediate writer事务中规范化与校验，单次提交revision及operation receipt。任一失败不保存任何新项，取消不调用命令。旧SaveGame单项编辑/CAS行为保持。
2. 批内重复itemId、明确运行环境/Bottle/规范路径的同一目标拒绝。与已有未归档记录身份相同则复用已有gameId/rowRevision，不覆盖名称、目标或专用配置；原稿“重复目标已复用”语义保留。未知/未解析身份不按裸文件名猜合并。新项稳定ID由operation+itemId派生，原operation重放返回原结果，不重复写。
3. CommandReceipt追加仅批次时序列化的gameResults[{itemId,gameId,rowRevision,reused}]；旧JSON空列表不序列化且反序列化默认空，以保旧回执。
4. TargetCandidate只投影真实已核验WindowTarget的windowHandle，可空macosHostPid/macosWindowId。当前底层没有Mac宿主身份时必须None，UI显式未取得；不得把Windows PID/HWND当Mac身份或按标题猜关联。公开DTO/schema/TS产物按同一源生成，Core ABI/protocol和DDL不改。
5. 游戏参数已有GameInput.target.arguments:string[]，UI新增逐项参数输入、保留空格/顺序、取消保草稿，不重新设计命令行shell解析。设置导航按现行明确十类规范恢复，后续范围用说明表达，不能伪造可用能力。

## 允许修改与边界

- 后端：crates/app-core/src/domain/library/mod.rs（共享normalize_game_batch复用既有normalize_game_input/validate_game，避免两层复制）、crates/app-core/src/contracts/{mod,desktop}.rs、application/{mod,runtime}.rs（runtime只补receipt默认字段）、crates/app-infra/src/storage/queries/{mod,tasks}.rs（tasks只补receipt默认字段）、hook_backend.rs、对应Rust测试；Tauri ipc命令角色match；由生成器输出schemas/application和contracts.generated.ts。
- 前端：现行LibraryScreen/CaptureSheet/SettingsWindow及相应样式/既有公共组件，复用ApplicationPort与生成DTO，不直调HTTP/SQL/FFI。
- 不改生产database.rs、DDL、资源锁/语言/Core；受保护source或契约生成器若必改，先独立审阅再新revision登记，禁止自动刷新。
- 当前associate未知gameId未入库/部分窗口身份核验等原实现缺口单独登记，不能把这次可空DTO投影当Mac映射成功或完整GL验收通过；本项不隐式扩大到重建native映射端口。

## 必需测试

空/超限/重复项、无expectedRevision/陈旧CAS、批内无部分提交、已有目标复用不覆写、跨Bottle独立身份、稳定顺序/结果、operation重放与冲突、单项SaveGame回归；所有事务在实际SqliteStore上执行。候选真实Win32字段/可空Mac投影，禁止猜值。

UI保留全部既有行为用例的业务断言；实际按设计修复后可独立审阅更新控件定位，禁止aria-label偷换、删断言或改预期掩盖行为。增加批次切换/取消零写入/原子保存失败、参数数组保真测试。
