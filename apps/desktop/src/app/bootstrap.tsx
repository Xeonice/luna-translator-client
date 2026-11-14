import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { windowRoleFromUrl } from './navigation';
import { ApplicationStore } from '../bridge/projections';
import { TauriApplicationPort } from '../bridge/tauri-port';
import { WordCardSurface } from '../features/language';
import { WindowRoot } from './WindowRoot';
import '../ui/reset.css';

export function bootstrap() {
  const transient = new URL(location.href).searchParams.get('surface') === 'word-card';
  const role = windowRoleFromUrl(location.href);
  document.documentElement.dataset.windowRole = transient ? 'word-card' : role;
  const container = document.getElementById('root');
  if (!container) throw new Error('缺少窗口挂载节点');
  const port = new TauriApplicationPort();
  const store = new ApplicationStore(port);
  createRoot(container).render(<StrictMode>{transient ? <WordCardSurface port={port}/> : <WindowRoot role={role} store={store}/>}</StrictMode>);
}

bootstrap();
