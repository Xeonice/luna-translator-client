# 本机翻译 worker

本目录是 CP05 的独立执行实现。业务队列、授权、历史、缓存、回退和提交仍由 Rust Runtime/Core/Repository 掌握；worker 不连接 HTTP，不进入游戏／Hook／MeCab 进程。原项目 License 和第三方材料保留。

## 真实运行边界

| 引擎 | 已实现路线 | 启用前提 |
|---|---|---|
| Apple | macOS26.4+，独立系统 Swift helper；`LanguageAvailability(.lowLatency)`与`TranslationSession(installedSource:…preferredStrategy:.lowLatency)` | 对应系统API、同策略语言已安装、实际OS build匹配；低版本 typed unsupported |
| Argos | 外部用户配置的Python运行器 + 固定 Argos1.11.0；包内 Stanza 分句、tokenizer、CTranslate2、段落合并；逐跳明确调用 | 解释器及脚本hash、完整语言包／Stanza资源、依赖版本观察与受审runtime profile；不随包嵌入解释器或权重 |
| J北京 | 独立Windows x86 worker，Unicode cdecl ABI；日→简中936／繁中950 | 用户合法安装、匹配的DLL/完整数据hash与位数；供应商版本资格单独实测 |
| 金山快译 | 独立Windows x86 worker，stdcall FastAIT ABI，选定GTS语言方向及DCT | 同上；Windows和CrossOver分别验证，不推断其普遍兼容 |

`ResourceManifest.engineVersion`对Apple是实际OS build，对Argos是运行库1.11.0，对商业引擎是**adapter ABI profile标识**，不是伪造的供应商版本。商业软件精确文件内容由hash绑定；系统或厂商未提供可观察资源版本时`resourceVersion=null`，UI必须解释“未提供”，不得显示假版本。Root资源Repository仍需绑定安装、环境、runtime profile、snapshot、route、observation及lease。

Apple headless会话不能下载语言。准备操作需独立系统交互host／明确用户许可；本helper不调用`prepareTranslation()`，不挂载SwiftUI、不静默改成highFidelity。缺包返回supportedNotInstalled/missing_resource。官方：[初始化](https://developer.apple.com/documentation/translation/translationsession/init(installedsource:target:preferredstrategy:))、[lowLatency](https://developer.apple.com/documentation/translation/translationsession/strategy/lowlatency)、[同策略可用性](https://developer.apple.com/documentation/translation/languageavailability/init(preferredstrategy:))。

Argos不会调用`get_translation`的自动中转、package index或下载API；只加载显式route中的PackageTranslation。1.11.0默认MiniSBD会下载模型／可能退到英文，因此本路线固定STANZA，并在原PackageTranslation内预建包目录中的`stanza.Pipeline(download_method=None)`。保留原分句／分词／推理／段落流水线，不仅调用裸CTranslate2；Python socket连接被拒绝。依赖运行库版本在Ready里实际读取，不能把“导入成功”当语言质量通过。官方：[1.11.0](https://pypi.org/project/argostranslate/1.11.0/)、[固定流水线源码](https://github.com/argosopentech/argos-translate/blob/v1.11.0/argostranslate/translate.py)、[Stanza分句源码](https://github.com/argosopentech/argos-translate/blob/v1.11.0/argostranslate/sbd.py)。Argos上游MIT；脚本是本项目GPL实现，实际第三方运行器／包许可仍须独立登记。

商业ABI只参考LunaTranslator固定commit`31b502ebf34c35d27276a3d5aaf58ca7b45f0f3b`的[jbj7.cpp](https://github.com/HIllya51/LunaTranslator/blob/31b502ebf34c35d27276a3d5aaf58ca7b45f0f3b/src/NativeImpl/LunaSubprocess/jbj7.cpp)与[kingsoft.cpp](https://github.com/HIllya51/LunaTranslator/blob/31b502ebf34c35d27276a3d5aaf58ca7b45f0f3b/src/NativeImpl/LunaSubprocess/kingsoft.cpp)。不猜`CloseEngine`／销毁导出或返回码含义；不借存在源码声称免费／兼容／可分发。DLL有固定缓冲ABI，输入上限依原实现（JB2999、FastAIT511 UTF16 units），未知扩容协议直接拒绝。实际vendor二进制内存行为与支持版本必须真实资格验证，子进程边界用于隔离崩溃，不等于厂商内存安全证明。

CrossOver只接显式launcher、Bottle、worker与Windows资源目录映射；父进程核Mac文件hash，worker核映射目录同一文件hash，不猜Z盘，不改系统／游戏窗口或Bottle状态。运行环境路径映射仍须Root从授权闭包取得。

## 私有协议和取消

`translation-worker-v1`有界JSONL，与Hook v2及language-worker v1不同。Hello固定engine/contract、request、worker instance、可选执行identity和资源manifest；Ready给实际环境及可用状态；Execute绑定任务／attempt／profile generation／epoch／lease／原句revision与hash；Final必须完全匹配资源、route和原identity。探测没有伪造task／lease，且不返回译文成功。

父进程同一引擎单在途、不另建业务队列；进程stdin/stdout受控、stderr不保留。输入65536、输出262144、单帧1048576 bytes；资源最多16384文件、32GiB、4跳，所有预算独立于测试样例。路径越界、symlink、未知版本、未就绪、额外Final、错误实例、超时／EOF／崩溃和资源变化均拒绝。超时关闭输入；取消发送Cancel后关闭，250ms grace后仅回收所拥有Child，绝不按进程名／游戏PID杀进程。

当前worker不声称可在DLL/原生长调用内即时响应Cancel，Ready.physicalCancelSupported=false。父进程在取消／超时先阻止结果并回收该独立进程；API自身的取消和worker进程被终止是不同事实。结果提交仍需Root复核lease／epoch等权威。

## 编译与验证

- Swift：`xcrun swiftc -sdk <受锁26.4+SDK> -parse-as-library native/offline-translation/apple_translation.swift -o <输出>`；显式SDK不修改系统xcode-select。
- Windows：`cargo build -p offline-translation-worker --bin offline-translation-worker --target i686-pc-windows-gnullvm --locked --offline`。Mac编译同一bin返回typed Windows-required；不运行DLL。worker独立轻crate仅依赖serde_json/sha2，不带SQLite/Core；目标链接工具缺失只block链接，不伪造Windows通过。
- Python：语法编译无导入副作用；实际运行必须外部合法runtime与完整包。本项目不自动pip安装、下载／解包语言包或启动服务。
- `cargo test -p app-infra --locked --offline --test offline_translation_protocol --test offline_translation_resources --test offline_translation_supervisor`使用真实child故障注入，**只证明协议／资源／监督器**；fixture不是翻译器，不报告译文质量通过。

当前实现不能单独提升产品状态。Root接线／资源Repo／interactive Apple准备host、真实Argos包、商业DLL、断网质量、Windows与CrossOver、最终同包四游戏仍各自留真实运行记录。
