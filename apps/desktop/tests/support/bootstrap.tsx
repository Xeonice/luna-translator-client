import wallpaperUrl from '../../../../docs/design/first-product-v1/assets/wallpaper-frame.png?url';
import { StrictMode, useState, useSyncExternalStore } from 'react';
import { CaptureSheet } from '../../src/features/capture/CaptureSheet';
import { WordCardSurface } from '../../src/features/language';
import { createRoot } from 'react-dom/client';
import { ApplicationStore } from '../../src/bridge/projections/application-store';
import { WindowRoot } from '../../src/app/WindowRoot';
import { windowRoleFromUrl } from '../../src/app/navigation';
import { DesignPort } from './design-port';
import { ReviewedReadingProbe } from './ReviewedReadingProbe';
import { ReviewedSurfaceProbe } from './ReviewedSurfaceProbe';
import '../../src/ui/reset.css';

const params = new URLSearchParams(location.search);
const role = windowRoleFromUrl(location.href);
document.documentElement.dataset.windowRole = role;
const backdrop = document.createElement('div');
backdrop.setAttribute('aria-hidden', 'true');
backdrop.setAttribute('data-ui-underlay', '');
Object.assign(backdrop.style, { position: 'fixed', inset: '0', background: `#020c31 url(${wallpaperUrl}) center / cover no-repeat`, pointerEvents: 'none' });
document.body.prepend(backdrop);
const rootElement = document.getElementById('root');
if (rootElement) { Object.assign(rootElement.style, { width: role === 'settings' ? '1000px' : '1040px', height: '720px', zIndex: '1', left: '40px', top: '40px' }); rootElement.dataset.role = role; if (role === 'hud') { rootElement.style.width = '560px'; rootElement.style.height = '290px'; } else if (role === 'learning') { rootElement.style.width = '1040px'; rootElement.style.height = '760px'; } }
const port = new DesignPort(params.get('scenario') ?? 'empty');
// Test-owned pre-render transport setup; production bootstrap never imports it.
const initializeControlledPort = Reflect.get(window, '__initializeControlledPort');
if (typeof initializeControlledPort === 'function') initializeControlledPort(port);
Object.assign(window, { uiHarness: port });
const container = document.getElementById('root');
if (!container) throw new Error('Missing UI test root');
function ControlledWordCardSurface() {
  const card = useSyncExternalStore(port.subscribeWordCardSurface, port.getWordCardSurface);
  if (!card) return null;
  const rect = port.wordCardRect;
  // The browser viewport is this controlled host's available work area, not
  // the native HUD's clipping boundary. Real multi-window placement is tested
  // separately against monitor geometry and in the packaged application.
  const width = Math.min(360, innerWidth - 16); const height = Math.min(460, innerHeight - 16);
  const left = Math.max(8, Math.min(rect?.left ?? 8, innerWidth - width - 8));
  const top = (rect?.top ?? 0) >= height + 16 ? rect!.top - height - 8 : Math.min((rect?.top ?? 0) + (rect?.height ?? 0) + 8, innerHeight - height - 8);
  return <div data-testid="controlled-word-card-surface" style={{ position: 'fixed', width, height, left, top: Math.max(8, top), zIndex: 500 }}><WordCardSurface key={card.cardId} port={port}/></div>;
}
const reactRoot = createRoot(container);
const store = new ApplicationStore(port);
function ControlledCaptureLifecycle() {
  const [open, setOpen] = useState(true);
  return <><button onClick={() => setOpen(true)}>重新打开关联</button><CaptureSheet game={null} snapshot={port.state} store={store} open={open} onClose={() => setOpen(false)} onRead={() => {}}/></>;
}
const surface = Reflect.get(window, '__controlledCaptureOnly') === true ? <ControlledCaptureLifecycle/> : params.has('reviewedSurface') ? <ReviewedSurfaceProbe store={store} surface={params.get('reviewedSurface')!} longList={params.get('longList') === 'true'}/> : params.get('probe') === 'reading-controls' ? <ReviewedReadingProbe/> : <><WindowRoot role={role} store={store}/><ControlledWordCardSurface/></>;
reactRoot.render(Reflect.get(window, '__controlledStrictMode') === true ? <StrictMode>{surface}</StrictMode> : surface);

if (import.meta.hot) import.meta.hot.dispose(() => { reactRoot.unmount(); backdrop.remove(); });
