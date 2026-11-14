/** Independent fixed verification entry; normal production bootstrap never imports it. */
import { createRoot } from 'react-dom/client';
import { WindowRoot } from '../../../src/app/WindowRoot';
import { ApplicationStore } from '../../../src/bridge/projections/application-store';
import { getGeneralVisualContext, reportGeneralVisualReady } from './native-context';
import { GeneralVisualPort } from './general-port';
import { prepareGeneralVisualCase, collectGeneralVisualFacts, waitForVisual } from './state-contract';
import '../../../src/ui/reset.css';

async function render(): Promise<void> {
  const context = getGeneralVisualContext();
  if (context.side !== 'candidate') throw new Error('The production-component entry only renders a candidate');
  document.documentElement.dataset.windowRole = 'settings';
  document.documentElement.style.zoom = String(context.zoom);
  const port = new GeneralVisualPort(context);
  const store = new ApplicationStore(port);
  const target = document.getElementById('root');
  if (!target) throw new Error('The verification document has no root');
  createRoot(target).render(<WindowRoot role="settings" store={store}/>);
  await waitForVisual(() => [...document.querySelectorAll<HTMLButtonElement>('nav[aria-label="设置分类"] button')].some(button => button.textContent?.trim() === '通用'), 'real settings navigation');
  const destination = [...document.querySelectorAll<HTMLButtonElement>('nav[aria-label="设置分类"] button')].find(button => button.textContent?.trim() === '通用')!;
  destination.click();
  const actions = [{ action: 'click_navigation', target: '通用' }, ...await prepareGeneralVisualCase(context, port)];
  const facts = collectGeneralVisualFacts(context, port, actions);
  await reportGeneralVisualReady(facts);
}
void render().catch(async cause => {
  const error = cause instanceof Error ? cause.message : String(cause);
  try { await reportGeneralVisualReady({ errors: [error], renderSource: 'production-WindowRoot', provenance: { controlledTransport: true, osRuntimeVerified: false, storageRuntimeVerified: false, gameExecutionVerified: false } }, error); }
  catch { document.body.textContent = 'General visual verification could not start.'; }
});
