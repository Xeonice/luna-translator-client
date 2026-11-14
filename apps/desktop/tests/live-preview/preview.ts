import './preview.css';

const pages = [
  ['session', '当前会话'], ['library', '游戏库'], ['history', '历史记录'],
  ['manual', '手动翻译'], ['settings-general', '通用设置'],
  ['settings-translation', '翻译服务'], ['hud', '字幕浮窗'],
  ['learning', '查词与学习'], ['diagnostics', '诊断'],
] as const;
const states = [['normal', '有内容'], ['empty', '空状态'], ['disconnected', '取文已断开'], ['design-reference', '设计对照样例']] as const;
const sizes = [['normal', '已确认常态尺寸'], ['narrow', '窄窗 760 × 560']] as const;
const params = new URLSearchParams(location.search);
const valid = (key: string, values: readonly (readonly [string, string])[], fallback: string) => values.some(([id]) => id === params.get(key)) ? params.get(key)! : fallback;
let view = valid('view', pages, 'session');
let state = valid('state', states, 'normal');
let size = valid('size', sizes, 'normal');
let theme = params.get('theme') === 'light' ? 'light' : 'dark';
const root = document.getElementById('review-root');
if (!root) throw new Error('Missing live review host');
const options = (values: readonly (readonly [string, string])[]) => values.map(([id, label]) => `<option value="${id}">${label}</option>`).join('');
root.innerHTML = `<header><strong>Luna · 实时界面预览</strong><span class="badge">实际客户端组件 · 样例数据</span><label>页面 <select id="review-page" aria-label="预览页面">${options(pages)}</select></label><label>状态 <select id="review-state" aria-label="预览状态">${options(states)}</select></label><label>尺寸 <select id="review-size-control" aria-label="预览尺寸">${options(sizes)}</select></label><label>外观 <select id="review-theme" aria-label="预览外观"><option value="dark">深色</option><option value="light">浅色</option></select></label><button id="review-reset">重置样例</button><a id="review-standalone" target="_blank" rel="noopener">独立查看</a><span class="spacer"></span><output id="review-status">连接开发服务…</output></header><p class="explanation">修改客户端 TSX / CSS 并保存后，此处自动更新，无需重新打包。<span id="review-size"></span> 工具栏位于客户端视口之外。</p><main class="stage"><div class="viewport-box"><iframe id="review-client" title="Luna 实际客户端组件预览"></iframe></div></main><footer>样例只在内存中运行，不读取个人资料库，不调用模型、Keychain、Hook 或系统选窗。翻译服务目录尚未安装真实 Rust 表单 fixture 时会明确显示未就绪。浏览器不代替原生毛玻璃、窗口、权限和游戏回归。<br>固定入口 <code>http://127.0.0.1:1421/</code> · HMR 重载可能重置样例；当前页面、尺寸和主题保存在地址中。</footer>`;
const designSampleNote = document.createElement('strong'); designSampleNote.id = 'review-design-sample-note';
designSampleNote.textContent = ' 设计对照内存样例：同原稿台词、标题与译文；服务、资格与预览不代表实机。';
root.querySelector('.explanation')!.append(designSampleNote);
const select = (id: string) => document.getElementById(id) as HTMLSelectElement;
const frame = document.getElementById('review-client') as HTMLIFrameElement;
const status = document.getElementById('review-status')!;
const roleFor = (page: string) => page.startsWith('settings-') ? 'settings' : ['hud', 'learning'].includes(page) ? page : 'main';
function syncAddress() {
  const next = new URL(location.href);
  for (const [key, value] of Object.entries({ view, state, size, theme })) next.searchParams.set(key, value);
  history.replaceState(null, '', next);
  select('review-page').value = view; select('review-state').value = state;
  select('review-size-control').value = size; select('review-theme').value = theme;
  designSampleNote.hidden = state !== 'design-reference';
}
function display(reset = false) {
  syncAddress();
  const role = roleFor(view);
  const [width, height] = size === 'narrow' ? [760, 560] : role === 'settings' ? [1000, 720] : role === 'hud' ? [560, 290] : role === 'learning' ? [1040, 760] : [1040, 720];
  frame.width = String(width); frame.height = String(height);
  const url = new URL('client.html', location.href);
  for (const [key, value] of Object.entries({ role, view, state, theme })) url.searchParams.set(key, value);
  if (reset) url.searchParams.set('reset', crypto.randomUUID());
  frame.src = url.href;
  (document.getElementById('review-standalone') as HTMLAnchorElement).href = url.href;
  document.getElementById('review-size')!.textContent = `客户端真实视口：${width} × ${height} CSS px。`;
  status.textContent = '正在加载真实组件…';
}
select('review-page').addEventListener('change', event => { view = (event.target as HTMLSelectElement).value; display(); });
select('review-state').addEventListener('change', event => { state = (event.target as HTMLSelectElement).value; display(); });
select('review-size-control').addEventListener('change', event => { size = (event.target as HTMLSelectElement).value; display(); });
select('review-theme').addEventListener('change', event => { theme = (event.target as HTMLSelectElement).value; display(); });
document.getElementById('review-reset')!.addEventListener('click', () => display(true));
window.addEventListener('message', event => {
  if (event.origin !== location.origin || event.source !== frame.contentWindow) return;
  const message: unknown = event.data;
  if (!message || typeof message !== 'object' || !('kind' in message)) return;
  if (message.kind === 'luna-review-ready' || message.kind === 'luna-review-hmr') status.textContent = `${message.kind === 'luna-review-hmr' ? '热更新' : '组件已加载'} · ${new Date().toLocaleTimeString('zh-CN')}`;
  if (message.kind === 'luna-review-blocked-action') status.textContent = '预览不会打开外部网址';
  if (message.kind === 'luna-review-navigate' && 'view' in message && typeof message.view === 'string' && pages.some(([id]) => id === message.view)) { view = message.view; display(); }
  if (message.kind === 'luna-review-page' && 'view' in message && typeof message.view === 'string' && pages.some(([id]) => id === message.view)) { view = message.view; syncAddress(); }
});
display();
