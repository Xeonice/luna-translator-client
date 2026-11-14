# 客户端组件实时预览

从仓库根运行 `node scripts/dev/live-ui-preview.mjs`，打开固定入口 <http://127.0.0.1:1421/>。重复启动会报告当前服务，不会结束占用端口的程序。停止自己启动的服务可运行 `node scripts/dev/live-ui-preview.mjs --stop`；状态用 `--status`。

修改实际 `apps/desktop/src` 的 TSX / CSS 并保存，Vite 热更新即在此入口呈现，不需要 Tauri 编译或签名。壳、iframe 和样例组装位于此目录；实际产品仍使用原 `WindowRoot`、各窗口、业务组件和样式，没有复制设计稿。

外层工具栏不会挤占客户端视口：Main / 历史为 1040×720，设置为 1000×720，窄窗为 760×560；HUD 为 560×290，学习窗为 1040×760。iframe 原点为 0。可切换页面、样例有无内容、断开状态和明暗外观；URL 保留选择，重置按钮清除本次内存交互。

状态下拉中的“设计对照样例”可持续查看与冻结原稿相同的游戏标题、原文、分词及译文；直接入口为 <http://127.0.0.1:1421/?view=session&state=design-reference>。数据由 `design-reading-sample.ts` 以 typed 内存种子组装，原稿身份及原文SHA随种子声明，无 eval、个人资料读取或真实服务调用。示例当前服务只用于呈现，资格明确被阻断且未测试；窗口保持未关联、没有录屏权限或伪造游戏帧。字号偏好仍为原18/24，由生产Main呈现规则派生，不为截图手改偏好。该模式只辅助阅读对照，不代表任何实机服务、Hook、词典质量或预览能力已通过。

样例使用既有 `DesignPort`，不会读取个人数据库或调用真实账号、HTTP、Keychain、Hook、文件选择和系统窗口。已保存服务投影采用合法空集合，不授予任何服务可用；未安装真实 Rust 表单投影 fixture 的翻译服务配置会明确报未就绪。断开样例同时解除窗口绑定，不把等待连接画面冒充采集。外部资源连接由 CSP 拒绝，外部链接点击不会打开网址。这不是完整配置功能验收。原生毛玻璃、窗口、权限、持久化及游戏回归仍须在实际包验证。此入口和样例端口不被正式生产 bootstrap 导入。

验证入口：`node --test apps/desktop/tests/live-preview/verify-live-preview.mjs`；UI 入口类型检查：`node apps/desktop/node_modules/typescript/bin/tsc --noEmit -p apps/desktop/tests/live-preview/tsconfig.json`。检查在 `.build/live-ui-preview/verification-*` 的隔离副本启动独立端口，实际修改并恢复该副本的生产 CSS，验证 HMR 可见且文档未重新导航；保留截图、真实生产 bundle 不含审阅入口的构建回执，以及端口归属负例。测试不会停止已在运行的用户预览服务。
