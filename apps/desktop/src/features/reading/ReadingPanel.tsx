import { type ReactNode, Fragment, useCallback, useEffect, useRef, useState } from 'react';
import type { AppSnapshot, GameWindowBindingProjection, ReadingUnit, SessionDataset, TaskView } from '../../bridge/contracts.generated';
import type { ApplicationStore } from '../../bridge/projections';
import { useAction } from '../../bridge/projections';
import { errorMessage } from '../../bridge/application-port';
import { Button } from '../../ui/primitives/Controls';
import { EmptyState, Notice, PageHeader } from '../../ui/patterns/Patterns';
import { Icon } from '../../ui/icons/Icon';
import { SourceText } from '../language';
import { WordCard } from '../language';
import { WindowPreview } from '../capture';
import { ServiceSelector, serviceSelectionRefreshKey } from '../model-config';
import { SessionRecordPanel } from './SessionRecordPanel';
import { readingPartialText, useReadingUnitProjection } from './useReadingUnitProjection';
import type { ReadingProjectionView } from './useReadingUnitProjection';
import { readingOriginLabel, sessionPresentation } from './session-presentation';
import { mainReadingTypography } from './reading-typography';
import styles from './styles.module.css';

export function TranslationStack({ unit, fontPt, compact = false, emptyFallback = <p className={styles.unavailable}>尚无译文。翻译用途未配置、未启用或仍在处理时，原文保持可读。</p> }: { unit: ReadingUnit; fontPt: number; compact?: boolean; emptyFallback?: ReactNode }) {
  return <div className={`${styles.translations} ${compact ? styles.compact : ''}`} >{unit.translations.length ? unit.translations.map(translation => <article className={styles.translation} key={translation.translationId}><p style={{ fontSize: `${fontPt}px` }}>{translation.correction?.text ?? translation.text}</p>{!compact || unit.translations.length > 1 ? <small>{translation.providerLabel} · {translation.targetLanguage}{translation.correction ? ' · 人工修正' : ''}</small> : null}</article>) : emptyFallback}</div>;
}
export function ReadingPanel({ snapshot, store, onStreams, onConnect, active = true }: { snapshot: AppSnapshot; store: ApplicationStore; onStreams: () => void; onConnect: () => void; active?: boolean }) {
  const action = useAction();
  const datasets: { id: SessionDataset; label: string }[] = [{ id: 'body', label: '正文历史' }, { id: 'extraction', label: '全部提取' }, { id: 'hook_events', label: 'Hook 日志' }];
  const session = snapshot.session;
  const [binding, setBinding] = useState<GameWindowBindingProjection | null>(null);
  const [bindingError, setBindingError] = useState<string | null>(null);
  const [activationNotice, setActivationNotice] = useState<string | null>(null);
  const bindingGeneration = useRef(0);
  const latestBinding = useRef<GameWindowBindingProjection | null>(null);
  const bindingPending = useRef<{ sessionId: string; generation: number; promise: Promise<void> } | null>(null);
  const sessionId = session?.sessionId ?? null;
  const renderedContext = useRef({ sessionId, active });
  renderedContext.current = { sessionId, active };
  const refreshBinding = useCallback(async (waitForPrevious = false) => {
    if (!active || !sessionId || renderedContext.current.sessionId !== sessionId || !renderedContext.current.active) return;
    const generation = bindingGeneration.current;
    const pending = bindingPending.current;
    if (pending?.sessionId === sessionId && pending.generation === generation) {
      if (!waitForPrevious) return;
      await pending.promise;
      if (generation !== bindingGeneration.current || renderedContext.current.sessionId !== sessionId || !renderedContext.current.active) return;
    }
    if (bindingPending.current?.sessionId === sessionId && bindingPending.current.generation === generation) return;
    const request = { sessionId, generation, promise: Promise.resolve() };
    // Only a request from this exact rendered context can block its next read.
    // An unrelated old session may never settle; it does not own the new one.
    bindingPending.current = request;
    request.promise = (async () => {
      try {
        const value = await store.port.gameWindowBinding();
        if (bindingPending.current !== request || generation !== bindingGeneration.current || renderedContext.current.sessionId !== sessionId || !renderedContext.current.active) return;
        if (value.sessionId && value.sessionId !== sessionId) throw new Error('窗口回执不属于当前会话，请重新核对。');
        const previous = latestBinding.current;
        if (value.state === 'unavailable' && ['window_proof_busy', 'preview_verification_unavailable', 'preview_capture_busy', 'preview_capture_timeout', 'preview_worker_transport'].includes(value.errorCode ?? '') && previous?.state === 'bound' && previous.sessionId === value.sessionId && previous.targetIdentitySha256 === value.targetIdentitySha256) {
          // A concurrent preview attestation is neither a new verification nor
          // an invalidation of the existing lease. Preserve the last verified
          // binding; a subsequent serial poll will obtain a fresh receipt.
          setBindingError('窗口正在重新核对，暂保留上次确认的关联。');
          return;
        }
        latestBinding.current = value; setBinding(value); setBindingError(null);
      } catch (cause) {
        if (bindingPending.current !== request || generation !== bindingGeneration.current || renderedContext.current.sessionId !== sessionId || !renderedContext.current.active) return;
        const previous = latestBinding.current;
        const transient = cause && typeof cause === 'object' && 'code' in cause && cause.code === 'capture_cleanup_pending' && 'stage' in cause && cause.stage === 'capture' && 'recoverable' in cause && cause.recoverable === true;
        if (transient && previous?.state === 'bound' && previous.sessionId === sessionId && previous.targetIdentitySha256) {
          setBindingError('窗口正在重新核对，暂保留上次确认的关联。');
          return;
        }
        latestBinding.current = null; setBinding(null); setBindingError(errorMessage(cause));
      }
    })();
    try { await request.promise; }
    finally { if (bindingPending.current === request) bindingPending.current = null; }
  }, [active, sessionId, store]);
  useEffect(() => {
    bindingGeneration.current += 1;
    latestBinding.current = null; setBinding(null); setBindingError(null); setActivationNotice(null);
    if (active && sessionId) void refreshBinding(true);
    const timer = active && sessionId ? setInterval(() => void refreshBinding(), 5000) : undefined;
    return () => { bindingGeneration.current += 1; if (timer) clearInterval(timer); };
  }, [active, sessionId, refreshBinding]);
  const bound = binding?.state === 'bound' && binding.sessionId === sessionId;
  const showGame = () => void action.run(async () => {
    const generation = bindingGeneration.current;
    const targetHash = binding?.targetIdentitySha256;
    if (!bound || !sessionId || !targetHash) throw new Error('请先核对当前游戏窗口。');
    try {
      const receipt = await store.port.showAssociatedGame({ expectedSessionId: sessionId, expectedTargetIdentitySha256: targetHash });
      const currentBinding = latestBinding.current;
      if (generation !== bindingGeneration.current || renderedContext.current.sessionId !== sessionId || !renderedContext.current.active || currentBinding?.state !== 'bound' || currentBinding.sessionId !== sessionId || currentBinding.targetIdentitySha256 !== targetHash) return;
      if (receipt.sessionId !== sessionId || receipt.targetIdentitySha256 !== targetHash) throw new Error('显示回执不属于当前关联目标，请重新核对窗口。');
      setActivationNotice(receipt.state === 'activated' ? null : '游戏窗口已显示，但系统未允许切到前台。请手动点击游戏窗口。');
    } catch (cause) { if (generation === bindingGeneration.current && renderedContext.current.sessionId === sessionId && renderedContext.current.active) throw cause; }
    finally { if (generation === bindingGeneration.current && renderedContext.current.sessionId === sessionId && renderedContext.current.active) await refreshBinding(true); }
  });
  const unbind = () => void action.run(async () => {
    const generation = bindingGeneration.current;
    const targetHash = latestBinding.current?.targetIdentitySha256;
    const current = () => generation === bindingGeneration.current && renderedContext.current.sessionId === sessionId && renderedContext.current.active && latestBinding.current?.state === 'bound' && latestBinding.current.targetIdentitySha256 === targetHash;
    try {
      if(!sessionId || !targetHash)throw new Error('请先核对当前游戏窗口。');
      await store.command({ kind: 'unbind_window', payload: { expectedSessionId: sessionId, expectedTargetIdentitySha256: targetHash } });
      if (!current()) return;
      bindingGeneration.current += 1; latestBinding.current = null; setBinding(null); setActivationNotice(null);
      await refreshBinding(true);
    } catch (cause) { if (current()) throw cause; }
  });
  const captureState = sessionPresentation(session);
  const stopped = session !== null && !captureState.live;
  const game = snapshot.games.find(item => item.gameId === session?.gameId);
  const environment = game?.target.environmentKind === 'crossover' ? 'CrossOver' : game?.target.environmentKind === 'wine' ? 'Wine' : game?.target.environmentKind === 'native_mac' ? 'macOS' : game?.target.environmentKind;
  const bottle = game?.target.bottleLocator?.split(/[\/]/).at(-1);
  const executable = game?.target.canonicalExePath?.split(/[\\/]/).at(-1);
  const targetDetail = [environment, bottle, executable].filter(Boolean).join(' · ');
  const current = snapshot.current?.sessionId === session?.sessionId ? snapshot.current : snapshot.history.units.find(unit => unit.sessionId === session?.sessionId);
  return <div className={styles.sessionContent} data-ui-scroll="main-content" data-session-layout={stopped ? 'stopped' : 'live'}>
    <PageHeader title={game?.displayName ?? '当前会话'} description={stopped ? `${captureState.label} · 已保存的原文、译文和查词仍可使用` : targetDetail || captureState.label} actions={session ? stopped ? <><Button intent="primary" onPress={onConnect}>重新连接游戏…</Button><Button onPress={() => void action.run(() => store.port.openWindow('hud'))}>显示浮窗</Button></> : <><Button disabled={!bound} pending={action.pending} onPress={showGame}>显示游戏</Button><Button onPress={() => void action.run(() => store.port.openWindow('hud'))}>显示浮窗</Button><Button intent="danger" pending={action.pending} onPress={() => void action.run(() => store.command({ kind: 'disconnect' }))}>断开连接…</Button></> : undefined}/>
    {session ? <>
      <ServiceSelector appearance="session" port={store.port} profileGeneration={snapshot.profileGeneration} scope={session.gameId ? { kind: 'game', gameId: session.gameId } : { kind: 'global' }} purpose="translation" refreshKey={serviceSelectionRefreshKey(snapshot)} onConfigure={() => void action.run(() => store.port.openSettingsPage('translation'))} onSelected={() => { void store.refresh(); }}/>
      <div className={`${styles.readingGrid} ${stopped ? styles.stoppedGrid : ''}`} data-verified-window={bound}>
        <div className={styles.currentReading}>
          {action.error ? <Notice error>{action.error}</Notice> : null}
          {bindingError ? <Notice error={!bound}>{bound ? '窗口重新核对暂未完成：' : '窗口状态读取失败：'}{bindingError}<Button onPress={() => void refreshBinding()}>重新核对窗口</Button></Notice> : null}
          {activationNotice ? <Notice>{activationNotice}</Notice> : null}
          {current || !stopped ? <div className={`${styles.liveStatus} ${stopped ? styles.savedStatus : ''}`} role="status">
            {!stopped ? <span className={styles.liveDot} data-connected={session.state === 'active'}/> : null}
            <span>{stopped ? '最近保存的一句' : captureState.label}</span>
            <small>{stopped ? '已停止接收新台词' : session.sources.filter(source => source.selected).map(source => source.hookName || source.sourceId).join('、') || '等待选择正文流'}</small>
          </div> : null}
          {current ? <SessionOverview snapshot={snapshot} store={store} unit={current} active={active} onStreams={onStreams} stopped={stopped}/> : <>
            <EmptyState title={stopped ? '当前没有可显示的句子' : '等待游戏原文'} description={stopped ? '已保存内容可从下方正文历史或“历史记录”回看；连接已运行的游戏后继续接收新句。' : '在游戏中推进一段对白。收到候选后，选择包含正文的文本流。'} icon="message"/>
            <CurrentSentenceActions snapshot={snapshot} store={store} unit={null} onStreams={onStreams}/>
          </>}
        </div>
        {!stopped || bound ? <aside className={styles.previewColumn}>{bound ? <WindowPreview key={`${session.sessionId}:${binding.targetIdentitySha256}`} port={store.port} candidateId="" consumer="session" consumerId={session.sessionId} title={binding.windowTitle || game?.displayName || '当前关联窗口'} detail={[bottle,executable].filter(Boolean).join(' · ') || '当前会话的已验证绑定'} identityDetail={`会话 ${session.sessionId}`} targetFacts={{ origin: 'game_record', gameTitle: game?.displayName, windowTitle: binding.windowTitle ?? undefined, verifiedAtMs: binding.verifiedAtMs, executablePath: game?.target.canonicalExePath ?? undefined, environmentKey: game?.target.environmentKey ?? undefined, bottleLabel: bottle }} onReconnect={onConnect}/> : <section className={styles.unboundPreview} aria-label="运行窗口预览"><Icon name="window" size={26}/><strong>{binding?.state === 'stale' ? '关联窗口已失效' : '尚无可核验的关联窗口'}</strong><p>{binding?.detail ?? '关联窗口核验后自动预览。游戏取文与窗口画面分别保留。'}</p><Button onPress={onConnect}>关联并接入游戏窗口…</Button></section>}</aside> : null}
      </div>
      <div className={styles.recordSections} aria-label="本次会话数据">
        {datasets.map(item => <SessionRecordPanel key={`${session.sessionId}:${item.id}`} snapshot={snapshot} store={store} sessionId={session.sessionId} dataset={item.id} disclosure active={active}/>)}
        {!stopped || bound ? <details className={styles.connectionManagement}>
          <summary>窗口关联管理</summary>
          <div>
            <p>{bound ? binding.windowTitle || game?.displayName || '已关联运行窗口' : binding?.detail ?? '关联窗口核验后自动预览。'}{bound && targetDetail ? ` · ${targetDetail}` : ''}</p>
            <Button onPress={onConnect}>{bound ? '选择或更换游戏窗口…' : '关联并接入游戏窗口…'}</Button>
            {bound ? <Button pending={action.pending} onPress={unbind}>解除窗口关联</Button> : null}
          </div>
        </details> : null}
      </div>
    </> : <EmptyState title="从一句原文开始" description="先选择一款运行中的游戏，核对窗口后关联并接入。"><Button intent="primary" onPress={onConnect}>连接游戏…</Button></EmptyState>}
  </div>;
}

/** Translation ownership is shared by the independent current card and the
 * annotated reading surfaces used in HUD, history and study. */
function projectedTasks(view: ReadingProjectionView): TaskView[] {
  return view.value?.translationState.engines.flatMap(engine => engine.status.kind === 'current' ? [engine.status.task] : engine.status.kind === 'unordered' && engine.status.activeTask ? [engine.status.activeTask] : []) ?? [];
}

function SessionOverview({ snapshot, store, unit, active, onStreams, stopped }: { snapshot: AppSnapshot; store: ApplicationStore; unit: ReadingUnit; active: boolean; onStreams: () => void; stopped: boolean }) {
  const action = useAction();
  // This component is intentionally not keyed by the current sentence. A word
  // opened from an earlier sentence owns its frozen unit/revision and anchor.
  const [word, setWord] = useState<{ unit: ReadingUnit; index: number; anchor: HTMLButtonElement } | null>(null);
  const view = useReadingUnitProjection(snapshot, store, unit, active);
  const contentUnit = view.value && view.value.commitRevision > snapshot.commitRevision ? view.value.unit : unit;
  const tasks = projectedTasks(view);
  const pending = tasks.some(task => task.status === 'running' || task.status === 'queued');
  const failed = tasks.some(task => task.status === 'failed');
  const cancelled = tasks.some(task => task.status === 'cancelled');
  const engines = view.value?.translationState.engines ?? [];
  const completed = engines.length > 0 && engines.every(engine => engine.status.kind === 'current' && engine.status.task.status === 'completed');
  const uncertain = engines.some(engine => engine.status.kind === 'unordered' || engine.status.kind === 'unavailable');
  const provider = contentUnit.translations.map(translation => translation.providerLabel).join('、');
  const singleTranslation = contentUnit.translations.length === 1 && contentUnit.translations[0].providerLabel ? contentUnit.translations[0] : null;
  const status = view.loading ? '读取翻译状态中' : view.error || view.refreshing ? '状态更新中' : uncertain ? '状态待核验' : pending ? '接收译文中' : failed ? '部分翻译未完成' : cancelled ? '翻译已取消' : completed ? '已完成' : '当前原句';
  const preferences = snapshot.preferences;
  const typography = mainReadingTypography(preferences);
  const sourcePreferences = { ...preferences, sourceFontPt: typography.sourceFontPx };
  const lexicalState = snapshot.lexicalStates.find(state => state.unitId === contentUnit.unitId && state.textRevision === contentUnit.textRevision && state.sourceHash === contentUnit.sourceHash);
  return <>
    <article className={`${styles.overview} ${stopped ? styles.savedOverview : ''}`} aria-label="当前原文与译文" data-guided-current="true">
      {action.error ? <Notice error>{action.error}</Notice> : null}
      <SourceText unit={contentUnit} preferences={sourcePreferences} lexicalState={lexicalState}
        onRetry={() => void action.run(() => store.port.retryLexical(contentUnit.unitId, contentUnit.textRevision))}
        onToken={(index, anchor) => setWord({ unit: structuredClone(contentUnit), index, anchor })}/>
      <TranslationStack unit={contentUnit} fontPt={typography.translationFontPx} compact={singleTranslation !== null} emptyFallback={null}/>
      <TranslationStatusContent snapshot={snapshot} store={store} view={view} translationFontPx={typography.translationFontPx} emptyUnit={!contentUnit.translations.length ? contentUnit : undefined}/>
      {stopped && (failed || engines.some(engine => engine.status.kind === 'unavailable')) ? <div className={styles.translationRecovery}><Button intent="quiet" onPress={() => void action.run(() => store.port.openSettingsPage('translation'))}>翻译设置…</Button></div> : null}
      <footer><span>{readingOriginLabel(contentUnit)} · {new Date(contentUnit.createdAtMs).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', hour12: false })}{provider ? ` · ${provider}` : ''}{singleTranslation ? ` · ${singleTranslation.targetLanguage}${singleTranslation.correction ? ' · 人工修正' : ''}` : ''}</span><span>{stopped ? '上次会话 · 非实时' : status}</span></footer>
    </article>
    <CurrentSentenceActions snapshot={snapshot} store={store} unit={contentUnit} onStreams={onStreams}/>
    {word ? <WordCard key={`${word.unit.unitId}:${word.unit.textRevision}:${word.index}`} unit={word.unit} tokenIndex={word.index} anchor={word.anchor} store={store} onClose={() => setWord(null)}/> : null}
  </>;
}

/** This order is the daily reading journey. Each command freezes the rendered
 * unit/revision at click time; control and display operations remain independent. */
function CurrentSentenceActions({ snapshot, store, unit, onStreams }: { snapshot: AppSnapshot; store: ApplicationStore; unit: ReadingUnit | null; onStreams: () => void }) {
  const sentenceAction = useAction(), captureAction = useAction(), navigationAction = useAction();
  const session = snapshot.session;
  const capture = sessionPresentation(session);
  const stopped = session !== null && !capture.live;
  if (stopped && !unit) return null;
  return <>
    <div className={styles.currentActions} role="group" aria-label="当前句操作">
      <Button disabled={!unit} pending={sentenceAction.pending} onPress={() => { if (unit) void sentenceAction.run(() => store.port.copyReadingUnit(unit.unitId, unit.textRevision)); }}>复制原文</Button>
      <Button disabled={!unit} pending={sentenceAction.pending} label="重新翻译本句" onPress={() => { if (unit) void sentenceAction.run(() => store.port.retranslate(unit.unitId, unit.textRevision, crypto.randomUUID())); }}>重新翻译</Button>
      {!stopped ? <><Button disabled={!capture.canPause} pending={captureAction.pending} onPress={() => { if (session) void captureAction.run(() => store.command({ kind: 'set_capture_paused', payload: { paused: !session.paused } })); }}>{session?.paused ? '继续自动输入' : '暂停自动输入'}</Button>
      <Button disabled={!capture.live} pending={captureAction.pending} onPress={() => { if (session) void captureAction.run(() => store.command({ kind: 'set_translation_enabled', payload: { enabled: !session.translationEnabled } })); }}>{session?.translationEnabled ? '关闭自动翻译' : '启用自动翻译'}</Button>
      <Button disabled={!capture.canSelectSources} onPress={onStreams}>重新选择文本流…</Button></> : null}
      <Button disabled={!unit} pending={sentenceAction.pending} label="分析本句文法" onPress={() => { if (unit) void sentenceAction.run(() => store.port.freezeLearning(unit.unitId, unit.textRevision, null)); }}>{stopped ? '精读这句' : '分析当前句'}</Button>
      {!stopped ? <Button pending={navigationAction.pending} onPress={() => void navigationAction.run(() => store.port.openMainPage('history'))}>查看历史</Button> : null}
    </div>
    {sentenceAction.error ? <Notice error>{sentenceAction.error}</Notice> : null}
    {captureAction.error ? <Notice error>{captureAction.error}</Notice> : null}
    {navigationAction.error ? <Notice error>{navigationAction.error}</Notice> : null}
  </>;
}

export function ReadingContent({ snapshot, store, unit, active = true, translationFontPx = snapshot.preferences.translationFontPt }: { snapshot: AppSnapshot; store: ApplicationStore; unit: ReadingUnit; active?: boolean; translationFontPx?: number }) {
  const action = useAction();
  const [word, setWord] = useState<{ unit: ReadingUnit; index: number; anchor: HTMLButtonElement } | null>(null);
  return <section className={styles.reading}><header><span>{readingOriginLabel(unit)} · {new Date(unit.createdAtMs).toLocaleTimeString('zh-CN', { hour12: false })}</span><div className={styles.actions}><Button icon="copy" intent="quiet" onPress={() => void action.run(() => store.port.copyReadingUnit(unit.unitId, unit.textRevision))}>复制原文</Button><Button icon="book" intent="quiet" onPress={() => void action.run(() => store.port.freezeLearning(unit.unitId, unit.textRevision, null))}>精读本句</Button></div></header>{action.error ? <Notice error>{action.error}</Notice> : null}<SourceText unit={unit} preferences={snapshot.preferences} lexicalState={snapshot.lexicalStates.find(state => state.unitId === unit.unitId && state.textRevision === unit.textRevision && state.sourceHash === unit.sourceHash)} onRetry={() => void action.run(() => store.port.retryLexical(unit.unitId, unit.textRevision))} onToken={(index, anchor) => setWord({ unit: structuredClone(unit), index, anchor })}/><TranslationStack unit={unit} fontPt={translationFontPx}/><TranslationTaskStates snapshot={snapshot} unit={unit} store={store} active={active} translationFontPx={translationFontPx}/>{word ? <WordCard key={`${word.unit.unitId}:${word.unit.textRevision}:${word.index}`} unit={word.unit} tokenIndex={word.index} anchor={word.anchor} store={store} onClose={() => setWord(null)}/> : null}</section>;
}

type TranslationQueryFeedback = 'notice' | 'layout-stable';

// Query-only feedback stays available to assistive technology without adding
// temporary measured content to a naturally sized HUD. Errors remain visible.
const layoutStableQueryFeedbackStyle = { position: 'absolute', width: 1, height: 1, padding: 0, margin: -1, overflow: 'hidden', clipPath: 'inset(50%)', whiteSpace: 'nowrap', border: 0 } satisfies import('react').CSSProperties;

export function TranslationTaskStates({ snapshot, unit, store, active = true, queryFeedback = 'notice', translationContent, translationFontPx }: { snapshot: AppSnapshot; unit: ReadingUnit; store: ApplicationStore; active?: boolean; queryFeedback?: TranslationQueryFeedback; translationFontPx?: number; translationContent?: { fontPt: number; compact?: boolean; render?: (unit: ReadingUnit, translation: ReactNode) => ReactNode } }) {
  const view = useReadingUnitProjection(snapshot, store, unit, active);
  // The hook validates the exact owner. Its request-start floor intentionally
  // permits as-of replies, so an older query must not erase newer snapshot text.
  const contentUnit = view.value && view.value.commitRevision > snapshot.commitRevision ? view.value.unit : unit;
  const translation = <>{translationContent ? <TranslationStack unit={contentUnit} fontPt={translationFontPx ?? translationContent.fontPt} compact={translationContent.compact} emptyFallback={null}/> : null}<TranslationStatusContent snapshot={snapshot} store={store} view={view} queryFeedback={queryFeedback} translationFontPx={translationFontPx} emptyUnit={translationContent && !contentUnit.translations.length ? contentUnit : undefined}/></>;
  return translationContent?.render ? translationContent.render(contentUnit, translation) : translation;
}

function TranslationStatusContent({ snapshot, store, view, queryFeedback = 'notice', emptyUnit, translationFontPx = snapshot.preferences.translationFontPt }: { snapshot: AppSnapshot; store: ApplicationStore; view: ReadingProjectionView; queryFeedback?: TranslationQueryFeedback; emptyUnit?: ReadingUnit; translationFontPx?: number }) {
  const action = useAction();
  const stale = view.refreshing || view.error !== null;
  const queryMessage = view.loading ? '正在读取翻译状态…' : view.refreshing ? '翻译状态更新中，以下保留上次读取结果。' : null;
  const staleLabel = stale ? queryFeedback === 'layout-stable' && !view.error ? <span style={layoutStableQueryFeedbackStyle}>上次读取 · </span> : '上次读取 · ' : null;
  const emptyEngines = emptyUnit && view.value ? view.value.translationState.engines : null;
  const automaticTranslationDisabled = emptyUnit?.sessionId != null && snapshot.session?.sessionId === emptyUnit.sessionId && !snapshot.session.translationEnabled;
  const taskRow = (task: TaskView, unordered = false) => {
    const partial = view.value ? readingPartialText(snapshot, view.value, task) : null;
    return <div className={styles.task} key={task.taskId}>{partial ? <p style={{ fontSize: `${translationFontPx}px` }}>{partial}</p> : null}<div><span>{staleLabel}{task.providerLabel} · {unordered ? '处理中请求：' : ''}{task.status === 'running' ? '翻译中' : task.status === 'queued' ? '等待翻译' : task.status === 'failed' ? `翻译失败：${task.errorCode ?? '请求未完成'}` : task.status === 'cancelled' ? '翻译已取消' : '旧翻译已失效'}</span>{task.status === 'running' || task.status === 'queued' ? <Button intent="quiet" onPress={() => void action.run(() => store.command({ kind: 'cancel_task', payload: { taskId: task.taskId } }))}>取消</Button> : null}</div></div>;
  };
  return <>{action.error ? <Notice error>{action.error}</Notice> : null}{queryFeedback === 'layout-stable' ? <span role="status" aria-label="翻译状态" aria-live="polite" aria-atomic="true" style={layoutStableQueryFeedbackStyle}>{view.error ? null : queryMessage}</span> : null}{view.error ? <Notice error>翻译状态读取失败：{view.error}<Button onPress={view.retry}>重新读取翻译状态</Button></Notice> : queryFeedback === 'notice' && queryMessage ? <Notice>{queryMessage}</Notice> : null}{view.value?.translationState.engines.map(engine => <Fragment key={JSON.stringify([engine.scopeKey, engine.bindingId, engine.engineId])}>{engine.status.kind === 'current' ? engine.status.task.status === 'completed' ? emptyUnit ? <p className={styles.unavailable}>{staleLabel}{engine.providerLabel} · 翻译请求已完成，当前没有可显示的译文。</p> : null : taskRow(engine.status.task) : engine.status.kind === 'unavailable' ? <Notice error>{engine.providerLabel} · 翻译状态无法核验：{engine.status.error.message}</Notice> : engine.status.kind === 'unordered' ? <><Notice>{engine.providerLabel} · 请求归属未定，共 {engine.status.summary.headCount} 次请求{engine.status.summary.failedCount !== '0' ? `，含 ${engine.status.summary.failedCount} 次失败请求` : ''}。原文与已有译文保留，可重新翻译。</Notice>{engine.status.activeTask ? taskRow(engine.status.activeTask, true) : null}</> : emptyUnit ? <p className={styles.unavailable}>{staleLabel}{engine.providerLabel} · 尚未发起翻译。</p> : null}</Fragment>)}{emptyEngines && (!emptyEngines.length || emptyEngines.some(engine => engine.status.kind === 'unrequested')) ? <div className={styles.unavailable}>{!emptyEngines.length ? <p>{staleLabel}{automaticTranslationDisabled ? '自动翻译已关闭，原文保持可读。' : '当前原句暂无翻译任务。'}</p> : null}<Button intent="quiet" onPress={() => void action.run(() => store.port.openWindow('settings'))}>检查翻译设置…</Button></div> : null}</>;
}
