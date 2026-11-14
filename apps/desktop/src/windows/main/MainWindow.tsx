import { useEffect, useRef, useState } from 'react';
import { errorMessage } from '../../bridge/application-port';
import type { AssociationAttributionIntent, Game } from '../../bridge/contracts.generated';
import type { ApplicationStore } from '../../bridge/projections';
import { useApplication } from '../../bridge/projections';
import { useAction } from '../../bridge/projections';
import { mainNavigation } from '../../app/navigation';
import type { MainPage } from '../../app/navigation';
import { WindowFrame, WindowLoading } from '../../ui/patterns/WindowFrame';
import { Icon } from '../../ui/icons/Icon';
import { Button } from '../../ui/primitives/Controls';
import { Notice } from '../../ui/patterns/Patterns';
import { LibraryScreen } from '../../features/library';
import { CaptureSheet } from '../../features/capture';
import { ReadingPanel, sessionPresentation } from '../../features/reading';
import { HistoryScreen } from '../../features/history';
import { ManualInputScreen } from '../../features/manual-input';
import { DiagnosticsScreen } from '../../features/diagnostics';
import styles from './styles.module.css';

export function MainWindow({ store }: { store: ApplicationStore }) {
  const { snapshot, error } = useApplication(store);
  const [page, setPage] = useState<MainPage>('library');
  const [capture, setCapture] = useState<{ game: Game | null; mode: 'target' | 'streams'; instance: string; attributionIntent: AssociationAttributionIntent; scopedGameId: string | null } | null>(null);
  const action = useAction();
  const [navigationError, setNavigationError] = useState<string | null>(null);
  const navigationRevision = useRef(0);
  useEffect(() => {
    let active = true; let dispose: (() => void) | undefined;
    navigationRevision.current = 0;
    void store.port.onMainPageRequested(request => {
      if (!active || request.revision <= navigationRevision.current) return;
      if (request.intent !== 'history' && request.intent !== 'session' && request.intent !== 'capture') return;
      navigationRevision.current = request.revision; setNavigationError(null);
      if (request.intent === 'history') { setCapture(null); setPage('history'); }
      else if (request.intent === 'session') { setCapture(null); setPage('session'); }
      else if (request.intent === 'capture') { setPage('library'); setCapture({ game: null, mode: 'target', instance: crypto.randomUUID(), attributionIntent: 'auto_match', scopedGameId: null }); }
    }).then(unsubscribe => { if (active) dispose = unsubscribe; else unsubscribe(); })
      .catch(error => { if (active) setNavigationError(errorMessage(error)); });
    return () => { active = false; dispose?.(); };
  }, [store]);
  const captureState = sessionPresentation(snapshot?.session ?? null);
  const sessionGame = snapshot?.games.find(game => game.gameId === snapshot.session?.gameId);
  const sessionTitle = sessionGame?.displayName ?? (snapshot?.session?.sourceKind === 'manual' ? '手动输入' : '未入库游戏');
  const connect = (game: Game | null, attributionIntent: AssociationAttributionIntent = game ? 'explicit_game' : 'auto_match', scopedGameId: string | null = game?.gameId ?? null) => setCapture({ game, mode: 'target', instance: crypto.randomUUID(), attributionIntent, scopedGameId });
  return <div className={styles.client}><WindowFrame title="Luna" tools={<div className={styles.windowTools}><Button icon="settings" intent="quiet" label="打开设置" onPress={() => void action.run(() => store.port.openWindow('settings'))}/></div>}>{snapshot ? <div className={styles.layout}><aside className={styles.sidebar}><nav aria-label="主导航">{['游戏', '文本', '工具'].map(group => <section key={group}><h2>{group}</h2>{group === '工具' ? <button onClick={() => void action.run(() => store.port.openWindow('settings'))}><Icon name="settings"/><span>设置</span></button> : null}{mainNavigation.filter(item => item.group === group).map(item => <button key={item.id} className={page === item.id ? styles.selected : ''} onClick={() => setPage(item.id)} aria-current={page === item.id ? 'page' : undefined}><Icon name={item.icon}/><span>{item.label}</span>{item.id === 'session' && captureState.live ? <i className={styles.sessionDot}/> : null}</button>)}</section>)}</nav><div className={styles.sidebarFooter}>{snapshot.session ? <><strong>{sessionTitle}</strong><small>{captureState.label}</small></> : <span>原文 · 翻译 · 理解</span>}</div></aside><main className={styles.content}>{error || action.error || navigationError ? <div className={styles.syncError}><Notice error>{navigationError ?? action.error ?? error}</Notice></div> : null}<div className={styles.featurePanel} hidden={page !== 'library'}><LibraryScreen active={page === 'library'} snapshot={snapshot} store={store} onConnect={() => connect(null)} onActivate={() => setPage('library')}/></div><div className={styles.featurePanel} hidden={page !== 'session'}><ReadingPanel active={page === 'session'} snapshot={snapshot} store={store} onConnect={() => connect(sessionGame ?? null, snapshot.session?.gameId ? 'explicit_game' : 'session_only', snapshot.session?.gameId ?? null)} onStreams={() => setCapture({ game: snapshot.games.find(game => game.gameId === snapshot.session?.gameId) ?? null, mode: 'streams', instance: crypto.randomUUID(), attributionIntent: snapshot.session?.gameId ? 'explicit_game' : 'session_only', scopedGameId: snapshot.session?.gameId ?? null })}/></div><div className={styles.featurePanel} hidden={page !== 'history'}><HistoryScreen snapshot={snapshot} store={store} active={page === 'history'}/></div><div className={styles.featurePanel} hidden={page !== 'manual'}><ManualInputScreen snapshot={snapshot} store={store} active={page === 'manual'}/></div><div className={styles.featurePanel} hidden={page !== 'diagnostics'}><DiagnosticsScreen snapshot={snapshot} store={store}/></div></main><CaptureSheet key={capture?.instance ?? 'closed'} game={capture?.game ?? null} attributionIntent={capture?.attributionIntent ?? 'auto_match'} scopedGameId={capture?.scopedGameId ?? null} mode={capture?.mode ?? 'target'} onRead={() => setPage('session')} snapshot={snapshot} store={store} open={capture !== null} onClose={() => setCapture(null)}/></div> : <WindowLoading error={error} onRetry={() => void store.refresh()}/>}</WindowFrame></div>;
}
