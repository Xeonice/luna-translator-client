/** Development-only assembly. Production bootstrap never imports this entry. */
import { createRoot } from 'react-dom/client';
import { WindowRoot } from '../../src/app/WindowRoot';
import { ApplicationStore } from '../../src/bridge/projections/application-store';
import { windowRoleFromUrl } from '../../src/app/navigation';
import { DesignPort } from '../support/design-port';
import { DESIGN_READING_SAMPLE, installDesignReadingSample } from './design-reading-sample';
import wallpaperUrl from '../../../../docs/design/first-product-v1/assets/wallpaper-frame.png?url';
import '../../src/ui/reset.css';

const params = new URLSearchParams(location.search);
const role = windowRoleFromUrl(location.href);
const view = params.get('view') ?? 'session';
const state = params.get('state') ?? 'normal';
document.documentElement.dataset.windowRole = role;
document.documentElement.dataset.liveUiPreview = 'sample-data';
Object.assign(document.documentElement.style, { overflow: 'hidden' });
Object.assign(document.body.style, { overflow: 'hidden', background: `#020c31 url(${wallpaperUrl}) center / cover no-repeat` });
const container = document.getElementById('root');
if (!container) throw new Error('Missing preview client root');
Object.assign(container.style, { position: 'absolute', inset: '0', width: '100%', height: '100%', minWidth: '0' });
const port = new DesignPort(state === 'empty' ? 'empty' : role === 'learning' ? 'learning' : 'reading');
port.state.preferences.theme = params.get('theme') === 'light' ? 'light' : 'dark';
// A genuine empty projection, not invented catalogue rows or service eligibility.
port.serviceFixtures.saved = async query => ({
  profileGeneration: port.state.profileGeneration,
  appliedHeadRevision: port.state.configuration.appliedHeadRevision,
  selection: { requestedScope: structuredClone(query.scope), effectiveScope: { kind: 'global' }, selectionRevision: 0, effectiveSelectionRevision: 0, main: null, comparisons: [], purposeEnabled: false },
  rows: [],
});
if (state === 'disconnected' && port.state.session) {
  port.state.session = { ...port.state.session, state: 'stopped' };
  port.bindingFixture = { state: 'unbound', sessionId: port.state.session.sessionId, targetIdentitySha256: null, windowTitle: null, verifiedAtMs: null, errorCode: null, detail: '受控断开样例：当前没有已确认的窗口关联。' };
  port.previewProjections.clear(); port.previewFrameFixtures.clear();
}
if (state === 'design-reference') await installDesignReadingSample(port);
const send = (message: Record<string, unknown>) => { if (parent !== window) parent.postMessage(message, location.origin); };
const blockExternalLink = (event: MouseEvent) => {
  const anchor = event.target instanceof Element ? event.target.closest('a[href]') : null;
  if (!(anchor instanceof HTMLAnchorElement)) return;
  const url = new URL(anchor.href, location.href);
  if (url.origin !== location.origin) { event.preventDefault(); send({ kind: 'luna-review-blocked-action' }); }
};
document.addEventListener('click', blockExternalLink, true);
document.addEventListener('auxclick', blockExternalLink, true);
port.openWindow = async requested => { send({ kind: 'luna-review-navigate', view: requested === 'settings' ? 'settings-translation' : requested === 'hud' || requested === 'learning' ? requested : 'session' }); };
const openTypedSettings = port.openSettingsPage.bind(port);
port.openSettingsPage = async intent => { await openTypedSettings(intent); send({ kind: 'luna-review-navigate', view: 'settings-translation' }); };
port.closeWindow = async () => { send({ kind: 'luna-review-navigate', view: 'session' }); };
Object.assign(window, { liveUiPreview: { kind: 'controlled-memory-only', port, source: 'production-WindowRoot', designSample: state === 'design-reference' ? { id: DESIGN_READING_SAMPLE.id, provenance: DESIGN_READING_SAMPLE.provenance } : null } });
const store = new ApplicationStore(port);
const reactRoot = createRoot(container);
reactRoot.render(<WindowRoot role={role} store={store}/>);
const mainLabels: Record<string, string> = { session: '当前会话', library: '游戏库', history: '历史记录', manual: '手动翻译', diagnostics: '诊断' };
let initialized = false;
let previousPage = '';
const observer = new MutationObserver(() => {
  const navigation = container.querySelector('nav');
  if (!navigation) return;
  if (!initialized) {
    const label = role === 'settings' ? view === 'settings-general' ? '通用' : '翻译服务' : mainLabels[view];
    const button = [...navigation.querySelectorAll('button')].find(node => node.textContent?.trim() === label);
    if (button) { initialized = true; button.click(); send({ kind: 'luna-review-ready' }); return; }
    else if (role !== 'main' && role !== 'settings') { initialized = true; send({ kind: 'luna-review-ready' }); }
  }
  if (role === 'main') {
    const selected = navigation.querySelector('[aria-current="page"]')?.textContent?.trim();
    const page = Object.entries(mainLabels).find(([, label]) => label === selected)?.[0];
    if (page && initialized && page !== previousPage) { previousPage = page; send({ kind: 'luna-review-page', view: page }); }
  }
});
observer.observe(container, { childList: true, subtree: true, attributes: true, attributeFilter: ['aria-current'] });
if (role === 'hud' || role === 'learning') queueMicrotask(() => send({ kind: 'luna-review-ready' }));
if (import.meta.hot) {
  import.meta.hot.on('vite:afterUpdate', () => send({ kind: 'luna-review-hmr' }));
  import.meta.hot.dispose(() => { observer.disconnect(); document.removeEventListener('click', blockExternalLink, true); document.removeEventListener('auxclick', blockExternalLink, true); reactRoot.unmount(); });
}
