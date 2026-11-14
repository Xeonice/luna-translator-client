import { useEffect, useRef, useState } from 'react';
import type { HudPreference, ReadingUnit, ShortcutIntent, ShortcutProjection, WindowCapabilities } from '../../bridge/contracts.generated';
import type { ApplicationStore } from '../../bridge/projections';
import { errorMessage } from '../../bridge/application-port';
import { useAction, useApplication } from '../../bridge/projections';
import { Button } from '../../ui/primitives/Controls';
import { ActionMenu } from '../../ui/primitives/Overlays';
import type { ActionMenuItem } from '../../ui/primitives/Overlays';
import { Notice } from '../../ui/patterns/Patterns';
import { WindowLoading } from '../../ui/patterns/WindowFrame';
import { SourceText, SourceTextStatus, WordCard } from '../../features/language';
import { TranslationTaskStates, sessionPresentation } from '../../features/reading';
import { ServiceSelector, serviceSelectionRefreshKey } from '../../features/model-config';
import styles from './styles.module.css';

export function HudWindow({ store }: { store: ApplicationStore }) {
  const { snapshot, error } = useApplication(store);
  const [word, setWord] = useState<{ unit: ReadingUnit; index: number; anchor: HTMLButtonElement } | null>(null);
  const action = useAction();
  const contentRef = useRef<HTMLDivElement>(null);
  const [fitError, setFitError] = useState<string | null>(null);
  const ready = !!snapshot;
  const [shortcuts, setShortcuts] = useState<ShortcutProjection | null>(null);
  const [shortcutsConfirmed, setShortcutsConfirmed] = useState(false);
  const shortcutRead = useRef<Promise<WindowCapabilities> | null>(null);
  const shortcutPreferences = JSON.stringify(snapshot?.preferences ?? null);
  // Native registration is independent of the current sentence. Refresh on
  // scope/config changes and focus, with a single bounded background read.
  useEffect(() => {
    if (!ready) return;
    let alive = true, busy = false;
    setShortcutsConfirmed(false);
    const refresh = async () => {
      if (!alive || busy) return;
      busy = true;
      try {
        if (shortcutRead.current) await shortcutRead.current.catch(() => undefined);
        if (!alive) return;
        const request = store.port.windowCapabilities(); shortcutRead.current = request;
        try { const value = await request; if (alive) { setShortcuts(value.shortcuts); setShortcutsConfirmed(true); } }
        catch { if (alive) setShortcutsConfirmed(false); }
        finally { if (shortcutRead.current === request) shortcutRead.current = null; }
      } finally { busy = false; }
    };
    const focused = () => { void refresh(); };
    const visible = () => { if (document.visibilityState === 'visible') void refresh(); };
    void refresh(); const timer = setInterval(focused, 5000);
    window.addEventListener('focus', focused); document.addEventListener('visibilitychange', visible);
    return () => { alive = false; clearInterval(timer); window.removeEventListener('focus', focused); document.removeEventListener('visibilitychange', visible); };
  }, [store, ready, snapshot?.session?.sessionId, shortcutPreferences]);
  const shortcutTitle = (label: string, intent: ShortcutIntent) => {
    if (!shortcutsConfirmed) return `${label} · 当前生效快捷键待核对`;
    const effective = shortcuts?.bindings.find(binding => binding.intent === intent)?.effective;
    if (!effective) return label;
    const symbols: Record<string, string> = { alt: '⌥', option: '⌥', super: '⌘', meta: '⌘', command: '⌘', cmd: '⌘', cmdorctrl: '⌘', commandorcontrol: '⌘', control: '⌃', ctrl: '⌃', shift: '⇧' };
    const combination = effective.split('+').map(part => symbols[part.trim().toLowerCase()] ?? part.trim().toUpperCase()).join(' ');
    return `${label} · ${combination}${shortcuts?.dispatchSuppressed ? '（正在录入，暂不响应）' : ''}`;
  };
  useEffect(() => {
    const content = contentRef.current; if (!content || !ready) return;
    let alive = true, inFlight = false, requested: number | null = null, pending: number | null = null;
    const flush = async () => {
      if (inFlight) return; inFlight = true;
      try {
        while (alive && pending !== null) {
          const height = pending; pending = null; if (height === requested) continue; requested = height;
          try { await store.port.fitHudContent(height); if (alive) setFitError(null); }
          catch (cause) { if (alive) setFitError(errorMessage(cause)); }
        }
      } finally { inFlight = false; }
    };
    const measure = () => {
      const reading = content.parentElement, hud = reading?.parentElement; if (!reading || !hud || !alive) return;
      const css = getComputedStyle(reading);
      const chrome = [...hud.children].filter(node => node.tagName === 'HEADER' || node.tagName === 'FOOTER').reduce((height, node) => height + node.getBoundingClientRect().height, 0);
      pending = Math.ceil(content.getBoundingClientRect().height + parseFloat(css.paddingTop) + parseFloat(css.paddingBottom) + chrome + 2);
      void flush();
    };
    const observer = new ResizeObserver(measure); observer.observe(content); measure();
    return () => { alive = false; pending = null; observer.disconnect(); };
  }, [store, ready]);
  const unit = snapshot?.current;
  const captureState = sessionPresentation(snapshot?.session ?? null);
  const compact = snapshot?.preferences.overrides['hud.compact'] ?? false;
  const pinned = snapshot?.preferences.overrides['hud.alwaysOnTop'] ?? true;
  const setHudPreference = (preference: HudPreference, enabled: boolean) => void action.run(() => store.command({ kind: 'set_hud_preference', payload: { preference, enabled } }));
  const menu: ActionMenuItem[] = snapshot ? [
    { id: 'settings', label: '设置…', action: () => void action.run(() => store.port.openWindow('settings')) },
    { id: 'bind', label: '选择或更换游戏窗口…', action: () => void action.run(() => store.port.openMainPage('capture')) },
    { id: 'unbind', label: '解除窗口关联', disabled: !snapshot.session, action: () => void action.run(async () => { const binding = await store.port.gameWindowBinding(); if(binding.state!=='bound'||binding.sessionId!==snapshot.session?.sessionId||!binding.sessionId||!binding.targetIdentitySha256)throw new Error(binding.errorCode==='window_proof_busy'?'窗口正在核对，请稍后重试解除关联。':'当前窗口关联已改变，请重新核对。'); await store.command({ kind:'unbind_window', payload:{expectedSessionId:binding.sessionId,expectedTargetIdentitySha256:binding.targetIdentitySha256} }); }) },
    { id: 'reset', label: '恢复浮窗位置', action: () => void action.run(() => store.port.resetHudPosition()) },
    { id: 'disconnect', label: '断开游戏连接…', separatorBefore: true, destructive: true, disabled: !captureState.live, action: () => void action.run(() => store.command({ kind: 'disconnect' })) },
  ] : [];
  if (!snapshot) return <div className={styles.hud} data-ui-surface="hud"><WindowLoading error={error} onRetry={() => void store.refresh()}/></div>;
  const preferences = snapshot.preferences;
  const mode = preferences.readingDisplayMode;
  const modeLabel = { both: '原文＋译文', source: '仅原文', translation: '仅译文' }[mode];
  // Only immutable object identity crosses an async command. The clipboard and
  // learning action must never re-read a later snapshot's current sentence.
  const copy = () => { if (unit) { const { unitId, textRevision } = unit; void action.run(() => store.port.copyReadingUnit(unitId, textRevision)); } };
  const study = () => { if (unit) { const { unitId, textRevision } = unit; void action.run(() => store.port.freezeLearning(unitId, textRevision, null)); } };
  const retranslate = () => { if (unit) { const { unitId, textRevision } = unit; void action.run(() => store.port.retranslate(unitId, textRevision, crypto.randomUUID())); } };
  const renderReadingContent = (contentUnit: ReadingUnit, translation: import('react').ReactNode) => {
    if (!unit) return null;
    const hasTranslation = contentUnit.translations.some(item => (item.correction?.text ?? item.text).length > 0);
    return <>
        {mode !== 'translation' || !hasTranslation ? <SourceText showStatus={false} unit={unit} preferences={{ ...preferences, sourceFontPt: preferences.sourceFontPt * (compact ? 16 / 18 : 1) }} lexicalState={snapshot.lexicalStates.find(state => state.unitId === unit.unitId && state.textRevision === unit.textRevision && state.sourceHash === unit.sourceHash)} onRetry={() => void action.run(() => store.port.retryLexical(unit.unitId, unit.textRevision))} onToken={(index, anchor) => setWord({ unit: structuredClone(unit), index, anchor })}/> : null}
        {mode !== 'source' ? translation : null}
        <SourceTextStatus unit={unit} preferences={preferences} lexicalState={snapshot.lexicalStates.find(state=>state.unitId===unit.unitId&&state.textRevision===unit.textRevision&&state.sourceHash===unit.sourceHash)} onRetry={()=>void action.run(()=>store.port.retryLexical(unit.unitId,unit.textRevision))}/>
        {mode === 'translation' && !hasTranslation ? <p className={styles.languageNote} role="status">当前尚无完整译文，继续保留原文。</p> : null}
    </>;
  };
  return <div className={`${styles.hud} ${compact ? styles.compact : ''}`} data-ui-surface="hud" style={{ '--hud-opacity': preferences.hudBackgroundOpacity } as import('react').CSSProperties}>
    <header className={styles.header} onPointerDown={event => {
      if (event.button !== 0 || event.target !== event.currentTarget) return;
      void action.run(() => store.port.beginHudDrag());
    }}>
      <div className={styles.dragHandle} title="拖动浮窗" role="presentation" onPointerDown={event => {
        if (event.button !== 0) return;
        event.preventDefault(); void action.run(() => store.port.beginHudDrag());
      }}/>
      <ServiceSelector appearance="hud" port={store.port} profileGeneration={snapshot.profileGeneration} scope={snapshot.session?.gameId ? { kind: 'game', gameId: snapshot.session.gameId } : { kind: 'global' }} purpose="translation" refreshKey={serviceSelectionRefreshKey(snapshot)} onConfigure={() => void action.run(() => store.port.openWindow('settings'))} onSelected={() => { void store.refresh(); }}/>
      <div className={styles.windowTools}>
        <Button intent="quiet" icon="pin" label={pinned ? '取消置顶' : '始终置顶'} pressed={pinned} onPress={() => setHudPreference('always_on_top', !pinned)}/>
        <Button intent="quiet" icon="sliders" label={compact ? '切换舒适布局' : '切换紧凑布局'} onPress={() => setHudPreference('compact', !compact)}/>
        <Button intent="quiet" icon="x" label="隐藏浮窗" title={shortcutTitle('隐藏浮窗', 'toggle_hud')} onPress={() => void action.run(() => store.port.closeWindow())}/>
      </div>
    </header>
    <section className={styles.reading} data-ui-scroll="reading">
      <div ref={contentRef} className={styles.readingContent}>
      {action.error || error ? <Notice error>{action.error ?? error}</Notice> : null}
      {unit ? <TranslationTaskStates snapshot={snapshot} unit={unit} store={store} active={mode !== 'source'} queryFeedback="layout-stable" translationContent={{ fontPt: preferences.translationFontPt * (compact ? 22 / 24 : 1), compact: true, render: renderReadingContent }}/> : <p className={styles.waiting}>等待游戏原文</p>}
      </div>
      {fitError ? <p className={styles.languageNote} role="status">浮窗尺寸调整未完成：{fitError}</p> : null}
    </section>
    <footer className={styles.toolbar} aria-label="阅读操作" onPointerDown={event => { if (event.button === 0 && event.target === event.currentTarget) void action.run(() => store.port.beginHudDrag()); }}>
      <span className={styles.state} role="status">{captureState.live && !snapshot.session?.paused ? '' : captureState.label}</span>
      <div className={styles.annotations} aria-label="字幕显示选项" onPointerDown={event => { if (event.button === 0 && event.target === event.currentTarget) void action.run(() => store.port.beginHudDrag()); }}>
        <Button intent="quiet" label="可点击分词" pressed={preferences.segmentationEnabled} onPress={() => setHudPreference('segmentation', !preferences.segmentationEnabled)}>分词</Button>
        <Button intent="quiet" label="假名注音" pressed={preferences.showRuby} onPress={() => void action.run(() => store.command({ kind: 'set_reading_annotations', payload: { showRuby: !preferences.showRuby, showPos: preferences.showPos } }))}>注音</Button>
        <Button intent="quiet" label="词性提示" title={preferences.segmentationEnabled ? '词性提示' : '启用分词后可显示词性提示'} pressed={preferences.showPos} disabled={!preferences.segmentationEnabled} onPress={() => void action.run(() => store.command({ kind: 'set_reading_annotations', payload: { showRuby: preferences.showRuby, showPos: !preferences.showPos } }))}>词性</Button>
        <Button intent="quiet" label={`字幕内容模式：${modeLabel}`} title={shortcutTitle('切换原文与译文显示', 'cycle_reading_display_mode')} onPress={() => void action.run(() => store.command({ kind: 'cycle_reading_display_mode' }))}>{modeLabel}</Button>
      </div>
      <span className={styles.divider} aria-hidden="true"/>
      <Button intent="quiet" icon="languages" label={snapshot.session?.translationEnabled ? '关闭自动翻译' : '启用自动翻译'} pressed={snapshot.session?.translationEnabled ?? false} disabled={!captureState.live} onPress={() => void action.run(() => store.command({ kind: 'set_translation_enabled', payload: { enabled: !snapshot.session?.translationEnabled } }))}/>
      <Button intent="quiet" icon={snapshot.session?.paused ? 'play' : 'pause'} label={snapshot.session?.paused ? '继续自动输入' : '暂停自动输入'} title={shortcutTitle(snapshot.session?.paused ? '继续自动输入' : '暂停自动输入', 'toggle_capture_pause')} pressed={snapshot.session?.paused ?? false} disabled={!captureState.canPause} onPress={() => void action.run(() => store.command({ kind: 'set_capture_paused', payload: { paused: !snapshot.session?.paused } }))}/>
      <Button intent="quiet" icon="history" label="重新翻译本句" disabled={!unit} onPress={retranslate}/>
      <Button intent="quiet" icon="copy" label="复制原文" disabled={!unit} onPress={copy}/>
      <Button intent="quiet" icon="sparkles" label="分析本句文法" disabled={!unit} onPress={study}/>
      <ActionMenu label="更多阅读操作" items={menu}/>
    </footer>
    {word ? <WordCard key={`${word.unit.unitId}:${word.unit.textRevision}:${word.index}`} unit={word.unit} tokenIndex={word.index} anchor={word.anchor} store={store} onClose={() => setWord(null)}/> : null}
  </div>;
}
