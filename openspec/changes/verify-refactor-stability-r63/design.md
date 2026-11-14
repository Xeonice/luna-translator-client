# 实际执行设计与 R65 rebase 边界

本工作项已在 R64 落入 Main；R64 的旧1943 snapshot与1950／2e265构建封印、worker漂移构建失败全部保留。本草案形成时R65派生资源候选尚待其独审、Root实际采用与完整检查。本次 rebase 不预造新 source/snapshot/包；只有这些前提实际发生后才在本工作项自有路径更换真实前像、修正文稿并重新冻结。

冻结后只使用同一源码、Core/语言资源、原开发证书、新 ZIP、安装 EXE/seal/签名以及 gate 新 nonce 冷启动一次 Main。顺序固定 SR 中文、MO 日语、Never7 日语、樱之诗 10th 日语，Ever17 不在范围；原 50 步原顺序、每步 600 秒、全轮 7200 秒、只读健康闭合等待 15 秒均保持。

每款 cold 确认未 Hook；connected 从实际入口关联并显式选择正文流；reading-1～5 每次推进一条不同真实正文后停住，同时取游戏/Main/HUD 原生截图与 AX 定位的预览独立 crop。主窗全文 OCR 不能代替预览 crop；成功元数据不能代替变化内容帧。日文 reading-5 同时给 UTF-8 token 范围、reading_kana、HUD 汉字注音与词典词卡；SR 不充当日文替身。

MO/Never7/樱之诗 wordcard-open 固定旧 unit/revision/token；wordcard-retained 只切回游戏推进长短句，触发真实 confirmed_content_fit，保留相同词卡窗口与旧对象，不点击 HUD、不切注音、不拖动。樱之诗 isolation-before/after 归属既有消费者切页、主窗正常关闭/从可见 HUD 原生菜单恢复，同时在同一已授权单窗 filter 的后台路径后恢复前台，保持相同 target/source/capture，不重选、不重启。此供给不新增步骤，也不把机器仅检查 source/capture 的证据夸大为 filter 指针证明；后台及恢复需留当次 OS 与原生诊断实际事实，缺真实条件即 blocked。

樱之诗 sleep-before 后必须等待 wake-after 的 pending-action barrier，再进行真实 OS Sleep/Wake；同 boot/Main/game/SID、源 producer、醒后新捕获帧与新正文由原校验器核对。显示器休眠、锁屏、DarkWake、旧日志均无效。四组 card-open/closed 分别验证 user-geometry、Escape、outside-pointer、owner-scroll 的真实 native_card_destroyed。

每款 exited 正常退出，Host EOF/ACK/final snapshot、数据库 stopped 与全部 operation stopped、owned Host 消失、目标 lock FD/inode 释放必须在下一款 cold 前实际取证。游戏 target 的切换落在这些退出与下款连接检查点，不能通过启动下款来代替旧 Host/租约释放。

动作回复只交本次 nonce/step/PID/实际窗口 ID，不交 pass 或预期台词。wrapper 复用原 `run-final-verify.py`（本次 owned verify.log 唯一末尾回执选择器），不用共享 latest 决定结果；storage/runtime/raw/recomputed 都须同 source/ZIP/nonce。任一不具备分别记 blocked/not_run，观察到错误记 failed，不能替换游戏、改 oracle 或复用旧包结果。


原50步不会显式触发 oversized geometry 或每一条 geometry-retention/recovery 分支。compatibility只读审阅55c9ef...仅证明collector/parser兼容：capture_recovery字段被保留计数，原generic oracle并不逐项断言post_proof/native_geometry/recovery。Sakura isolation和SleepWake成功仍各自只支持原source/capture、fresh-frame和正文结论，不能直接提升所有typed分支。

按既有原生合同另列每条实际附加场景的触发条件、same-attempt/epoch/原choice身份、geometry/native_geometry/failure_kind/post_proof/recovery原始事实、真正fresh guard→commit→delivery帧及OS可见结果；必须绑定相同最终source/package/Main/boot/nonce。仅有metadata或fixture通过不算实机；自然未触发写not_run，外部条件不足写blocked，真实拒绝/错误按预期与原合同判断。补证不删原50步、不把原预算延长，也不修改原oracle或新增私有调试入口。

实时操作沿用r2独审批准的pending-action/continue机械发现：从本次owned collector真实argv唯一定位新output，交叉绑定inputs/source/ZIP/Main PID/start/started时间/nonce/原step游标后才回四字段，确认action-NNN已消费才前进；LUNA_STABILITY_ACTION由上层PIPE缓存，只用于事后核对。wrapper字节仍为7a279213aa9abe88aa7088b829700d352d064e83c309039a612407ba08ed144a。运行配置和证据使用全新final-stability-r65路径，不写旧r64目录。
