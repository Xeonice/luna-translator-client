"""Offline SVG diagrams matching the architecture document's four Mermaid blocks."""

from html import escape


def canvas(key, title, description, width, height, body):
    return f'''<div class="diagram-scroll"><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" role="img" aria-labelledby="{key}-title {key}-desc">
<title id="{key}-title">{escape(title)}</title><desc id="{key}-desc">{escape(description)}</desc>
<defs><marker id="{key}-arrow" markerWidth="8" markerHeight="8" refX="7" refY="4" orient="auto-start-reverse"><path d="M0 0 L8 4 L0 8 Z" fill="#116e6b"/></marker></defs>
{body}</svg></div>'''


def text(x, y, value, cls="sv-label", anchor="middle"):
    return f'<text x="{x}" y="{y}" class="{cls}" text-anchor="{anchor}">{escape(value)}</text>'


def box(x, y, width, height, title, lines=(), kind="", badge=""):
    value = f'<rect class="sv-box {kind}" x="{x}" y="{y}" width="{width}" height="{height}" rx="9"/>'
    value += text(x + width / 2, y + 30, title, "sv-title")
    for index, line in enumerate(lines):
        value += text(x + width / 2, y + 54 + 20 * index, line)
    if badge:
        value += text(x + width / 2, y + height - 12, badge, "sv-small")
    return value


def arrow(key, x1, y1, x2, y2, label="", dashed=False, bidirectional=False):
    dash = ' stroke-dasharray="5 4"' if dashed else ""
    start = f' marker-start="url(#{key}-arrow)"' if bidirectional else ""
    value = f'<path class="sv-line" d="M{x1} {y1} L{x2} {y2}" marker-end="url(#{key}-arrow)"{dash}{start}/>'
    if label:
        value += text((x1 + x2) / 2, (y1 + y2) / 2 - 10, label)
    return value


def current_system():
    key = "current"
    body = box(25, 170, 190, 105, "CLI / 测试驱动", ["发命令 · 持续读输出"], badge="本次：无 attach")
    body += box(370, 160, 260, 125, "lunahook-host", ["Windows x64 Rust 进程", "命令 / 生命周期 / 事件"], "sv-native", "已实现 · 初始化已验证")
    body += box(785, 170, 190, 105, "LunaHost64.dll", ["保留的无视图 C++ Host"], "sv-native", "与 Rust 在同一宿主进程")
    body += box(370, 20, 260, 80, "固定运行时资源", ["10 文件 · SHA-256 / ABI 锁"], "sv-native")
    body += box(370, 365, 260, 70, "诊断消费者", ["持续读取 stderr"])
    body += arrow(key, 215, 197, 370, 197, "stdin JSONL")
    body += arrow(key, 370, 253, 215, 253, "stdout JSONL")
    body += arrow(key, 630, 197, 785, 197, "C API 调用")
    body += arrow(key, 785, 253, 630, 253, "C ABI 回调")
    body += arrow(key, 500, 100, 500, 160)
    body += text(605, 135, "加载前再次校验")
    body += arrow(key, 500, 285, 500, 365)
    body += text(655, 335, "原生调试输出与协议分离")
    return canvas(key, "当前最小系统", "测试驱动通过私有JSONL控制Rust宿主。宿主验证10个资源并加载LunaHost DLL，通过C ABI接收回调；诊断独立输出。游戏取文尚未验证。", 1000, 455, body)


def target_system():
    key = "target"
    body = '<rect class="sv-boundary" x="20" y="15" width="1040" height="260" rx="12"/>'
    body += text(45, 43, "目标：本机桌面应用组件（规划）；WebView 与 Rust 通过 Tauri IPC 交互", "sv-label", "start")
    body += box(50, 100, 245, 100, "React UI / WebView", ["HUD · 连接 · 设置", "独立内容与授权边界"], "sv-plan")
    body += box(400, 100, 245, 100, "Rust 应用核心", ["原生主进程 · 会话与输入", "无 UI 类型 / 平台句柄"], "sv-plan")
    body += box(750, 65, 280, 80, "翻译与文本流水线", ["术语 · 上下文 · provider"], "sv-plan")
    body += box(750, 175, 280, 80, "配置与历史适配器", ["结构化配置 / SQLite（建议）"], "sv-plan")
    body += arrow(key, 295, 125, 400, 125, "命令 / IPC")
    body += arrow(key, 400, 175, 295, 175, "领域事件")
    body += arrow(key, 645, 125, 750, 105)
    body += arrow(key, 645, 175, 750, 215)
    body += '<rect class="sv-boundary" x="20" y="340" width="650" height="295" rx="12"/>'
    body += text(45, 368, "Windows 环境；CrossOver 路线必须同一 bottle", "sv-label", "start")
    body += '<rect class="sv-boundary" x="255" y="385" width="390" height="230" rx="10"/>'
    body += text(275, 409, "同一个 Windows x64 宿主进程", "sv-small", "start")
    body += box(280, 430, 340, 70, "Rust 窄后端", ["身份 · 连接 · 状态 / 原文事件"], "sv-native")
    body += box(280, 535, 340, 60, "LunaHost DLL", ["原版 C++ 通信与文本流"], "sv-native")
    body += box(45, 430, 180, 70, "helper32 / 64", ["选择目标位数"], "sv-native")
    body += box(45, 535, 180, 60, "游戏 + Hook DLL", ["独立目标进程"], "sv-native")
    body += arrow(key, 280, 464, 225, 464)
    body += arrow(key, 135, 500, 135, 535)
    body += arrow(key, 225, 565, 280, 565)
    body += arrow(key, 450, 535, 450, 500)
    body += text(555, 524, "C ABI 回调")
    body += arrow(key, 520, 200, 520, 430, bidirectional=True)
    body += text(510, 300, "私有 stdin/stdout JSONL", "sv-label", "end")
    body += text(510, 324, "命令 ↓ / 事件 ↑", "sv-label", "end")
    body += box(750, 350, 280, 90, "原生 macOS 取字后端", ["独立提取入口 · 独立真实验收", "不能直接加载 Windows PE DLL"], "sv-plan")
    body += box(750, 490, 280, 90, "其他输入与平台适配", ["OCR / 剪贴板 / 手动 / 文件", "窗口 · 捕获 · 快捷键 · 凭据"], "sv-plan")
    body += f'<path class="sv-line" d="M750 395 L710 395 L710 225 L645 200" marker-end="url(#{key}-arrow)" stroke-dasharray="5 4"/>'
    body += f'<path class="sv-line" d="M750 535 L695 535 L695 245 L645 210" marker-end="url(#{key}-arrow)" stroke-dasharray="5 4"/>'
    return canvas(key, "目标产品组件与进程边界", "虚线蓝色为规划模块，绿色是已有宿主或保留的原生组件。应用核心通过私有JSONL接Windows宿主，也接独立原生Mac和其他输入后端。", 1080, 655, body)


def sequence(key, title, participants, messages, height):
    width = 1080
    xs = [75 + index * (930 / (len(participants) - 1)) for index in range(len(participants))]
    body = ""
    for x, (name, label) in zip(xs, participants):
        body += f'<rect class="sv-box" x="{x - 70}" y="15" width="140" height="62" rx="7"/>'
        body += text(x, 41, name, "sv-title") + text(x, 62, label, "sv-small")
        body += f'<line class="sv-guide" x1="{x}" y1="77" x2="{x}" y2="{height - 18}"/>'
    for index, (source, target, label, dashed) in enumerate(messages):
        y = 120 + index * 56
        if source == target:
            x = xs[source]
            body += f'<path class="sv-line" d="M{x} {y-9} L{x+45} {y-9} L{x+45} {y+12} L{x} {y+12}" marker-end="url(#{key}-arrow)"/>'
            body += text(x + 58, y + 4, label, "sv-seq", "start")
        else:
            body += arrow(key, xs[source], y, xs[target], y, dashed=dashed)
            body += text((xs[source] + xs[target]) / 2, y - 10, label, "sv-seq")
        body += text(6, y - 10, str(index + 1).zfill(2), "sv-num", "start")
    return canvas(key, title, "；".join(message[2] for message in messages), width, height, body)


def connection_sequence():
    return sequence("connection", "连接、注入、回调的独立阶段", [
        ("应用核心", "规划 / 驱动者"), ("Rust 宿主", "当前已实现"),
        ("LunaHost DLL", "C++ 原生"), ("注入器", "按目标位数"), ("目标游戏", "内含 Hook"),
    ], [
        (0, 1, "attach：PID + 路径 + 创建时间", False),
        (1, 0, "response：pending（尚未连接）", True),
        (1, 1, "校验身份、存活、架构", False),
        (1, 2, "ConnectProcess / CheckIfNeedInject", False),
        (1, 3, "若需要：dllinject + 固定 DLL 路径", False),
        (3, 4, "请求加载 Hook", False),
        (3, 1, "helper 退出 → injection_finished（仅一项证据）", True),
        (4, 2, "Hook 管道连接与消息", False),
        (2, 1, "Connect / HookInsert / Create / Output 回调", True),
        (1, 0, "分别发布进程、Hook、流、原文事件", True),
    ], 685)


def translation_sequence():
    return sequence("translation", "迟到译文不能覆盖新句（规划）", [
        ("输入适配器", "统一 InputEvent"), ("应用核心", "会话与修订"),
        ("翻译服务", "provider"), ("HUD", "当前显示槽"),
    ], [
        (0, 1, "原文 A：session + sentence + revision", False),
        (1, 3, "立即展示 A 原文", True),
        (1, 2, "请求 A + 规则 / 上下文快照", False),
        (0, 1, "原文 B：新句身份", False),
        (1, 3, "显示 B 原文", True),
        (2, 1, "A 的迟到译文", True),
        (1, 1, "保存 A，不覆盖 B", False),
        (1, 2, "请求 B", False),
        (2, 1, "B 的结果 + 原请求身份", True),
        (1, 3, "校验 generation / revision 后更新 B", True),
    ], 685)


def thread_diagram():
    key = "threads"
    body = box(10, 35, 225, 100, "native 回调线程", ["立即复制 · try_send", "不反调 Host / 不写 stdout"], "sv-native")
    body += box(285, 35, 175, 100, "回调队列", ["256 个事件", "满则计丢失"], "sv-native")
    body += box(515, 35, 230, 100, "supervisor", ["阶段 / 命令 / 超时 / 关闭", "有界排空，防控制饥饿"], "sv-native")
    body += box(815, 35, 235, 100, "输出队列 + writer", ["256 个事件 · 独立写线程", "协议 stdout"], "sv-native")
    body += arrow(key, 235, 85, 285, 85)
    body += arrow(key, 460, 85, 515, 85)
    body += arrow(key, 745, 85, 815, 85)
    body += box(10, 235, 225, 95, "原子连接状态", ["16 个固定槽", "与可丢文本队列分开"], "sv-native")
    body += box(285, 235, 220, 95, "native worker", ["连接 / 断开 / 自有 helper", "最多一个 attach 在途"], "sv-native")
    body += box(560, 235, 220, 95, "stdin 读取线程", ["帧限制 / 严格解析", "命令队列"], "sv-native")
    body += arrow(key, 120, 135, 120, 235)
    body += f'<path class="sv-line" d="M235 280 L255 280 L255 175 L565 175 L565 135" marker-end="url(#{key}-arrow)" stroke-dasharray="5 4"/>'
    body += text(395, 165, "控制状态补查")
    body += arrow(key, 670, 235, 670, 135)
    body += f'<path class="sv-line" d="M590 135 L590 195 L395 195 L395 235" marker-end="url(#{key}-arrow)"/>'
    body += text(940, 203, "输出阻塞：不反向等待回调")
    body += text(940, 227, "响应 / 错误也可能丢失")
    body += text(940, 251, "overflow 表示完整性失效")
    return canvas(key, "宿主线程与背压边界", "回调复制后经256容量队列到supervisor，再经256容量输出队列到独立writer。控制状态由16个原子槽保存，stdin与native worker分离。输出不是可靠投递。", 1060, 355, body)


DIAGRAMS = [current_system, target_system, connection_sequence, translation_sequence]
