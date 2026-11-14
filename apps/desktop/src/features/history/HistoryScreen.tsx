import { useEffect, useRef, useState } from 'react';
import type { AppSnapshot, HistoryFilter, HistorySessionFilter, HistorySessionSummary, ReadingUnit, Translation } from '../../bridge/contracts.generated';
import type { ApplicationStore } from '../../bridge/projections';
import { useAction } from '../../bridge/projections';
import { errorMessage } from '../../bridge/application-port';
import { Button, Select, TextField } from '../../ui/primitives/Controls';
import { EmptyState, Notice, PageHeader, StatusLabel } from '../../ui/patterns/Patterns';
import { Icon } from '../../ui/icons/Icon';
import { SourceText, WordCard } from '../language';
import { readingOriginLabel, TranslationStack, TranslationTaskStates } from '../reading';
import { useHistoryQuery } from './useHistoryQueries';
import styles from './styles.module.css';

function CorrectionEditor({ translation, store, onClose }: { translation: Translation; store: ApplicationStore; onClose: (saved: boolean) => void }) {
  const [text, setText] = useState(translation.correction?.text ?? translation.text);
  const [baseRevision] = useState(translation.correction?.revision ?? 0);
  const root = useRef<HTMLDivElement>(null);
  const action = useAction();
  useEffect(() => {
    const input = root.current?.querySelector('textarea');
    input?.focus({ preventScroll: true });
    input?.setSelectionRange(input.value.length, input.value.length);
  }, []);
  const save = async () => {
    const input = root.current?.querySelector('textarea');
    const selection = input ? { start: input.selectionStart, end: input.selectionEnd, direction: input.selectionDirection, scrollTop: input.scrollTop } : null;
    const saved = await action.run(async () => {
      await store.command({ kind: 'save_correction', payload: { translationId: translation.translationId, baseRevision, text } });
      onClose(true);
    });
    if (!saved && input?.isConnected && selection) {
      input.focus({ preventScroll: true });
      input.setSelectionRange(selection.start, selection.end, selection.direction);
      input.scrollTop = selection.scrollTop;
    }
  };
  return <div ref={root} className={styles.correction}>
    {action.error ? <Notice error>{action.error} 编辑内容已保留。</Notice> : null}
    <TextField label="人工修正" value={text} onChange={setText} multiline description="修正独立保存，原始模型结果与来源继续保留。"/>
    <div className={styles.actions}><Button disabled={action.pending} onPress={() => onClose(false)}>取消编辑</Button><Button intent="primary" pending={action.pending} onPress={() => void save()}>保存修正</Button></div>
  </div>;
}
function unitKey(unit: Pick<ReadingUnit, 'unitId' | 'textRevision'>) { return JSON.stringify([unit.unitId, unit.textRevision]); }
function dateLabel(value: number) { const date = new Date(value); return Number.isNaN(date.getTime()) ? '时间未记录' : date.toLocaleString('zh-CN', { hour12: false }); }
function shortDate(value: number) { const date = new Date(value); return Number.isNaN(date.getTime()) ? '时间未记录' : date.toLocaleString('zh-CN', { hour12: false, month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' }); }
function isoDate(value: number) { const date = new Date(value); return Number.isNaN(date.getTime()) ? undefined : date.toISOString(); }
function sessionStateLabel(state: string) { return ({ active: '阅读中', opening: '接入中', stopped: '已结束', failed: '接入失败', crashed: '已中断' } as Record<string, string>)[state] ?? '状态未知'; }
export function HistoryScreen({ snapshot, store, active = true }: { snapshot: AppSnapshot; store: ApplicationStore; active?: boolean }) {
  const owner = JSON.stringify([snapshot.profileId, snapshot.serverInstanceId, snapshot.profileGeneration]);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [gameFilter, setGameFilter] = useState('all');
  const [sessionFilter, setSessionFilter] = useState<string | null>(null);
  const [pinnedSession, setPinnedSession] = useState<HistorySessionSummary | null>(null);
  const [edit, setEdit] = useState<{ owner: string; unitKey: string; translation: Translation } | null>(null);
  const editing = edit?.owner === owner ? edit : null;
  const editingRef = useRef(editing); editingRef.current = editing;
  const [detailRefresh, setDetailRefresh] = useState(0);
  const [detail, setDetail] = useState<{ owner: string; unit: ReadingUnit } | null>(null);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [located, setLocated] = useState<{ owner: string; unit: ReadingUnit } | null>(null);
  const [word, setWord] = useState<{ owner: string; unit: ReadingUnit; index: number; anchor: HTMLButtonElement } | null>(null);
  const listScroll = useRef<HTMLDivElement>(null), detailScroll = useRef<HTMLElement>(null);
  const action = useAction();
  const gameId = gameFilter === 'all' || gameFilter === 'manual' ? null : gameFilter;
  const sourceKind = gameFilter === 'manual' ? 'manual' as const : null;
  const scope = JSON.stringify({ gameId, sourceKind, sessionId: sessionFilter } satisfies HistoryFilter);
  const catalogScope = JSON.stringify({ gameId, sourceKind } satisfies HistorySessionFilter);
  const records = useHistoryQuery<ReadingUnit>({
    owner, scope, active, blocked: editing !== null,
    fetchPage: async (requestedScope, cursor) => { const result = await store.port.historyPage({ filter: JSON.parse(requestedScope) as HistoryFilter, cursor, limit: 50 }); return { items: result.units, nextCursor: result.nextCursor }; },
    keyOf: unitKey,
  });
  const catalog = useHistoryQuery<HistorySessionSummary>({
    owner, scope: catalogScope, active, blocked: editing !== null,
    fetchPage: async (requestedScope, cursor) => { const result = await store.port.historySessions({ filter: JSON.parse(requestedScope) as HistorySessionFilter, cursor, limit: 50 }); return { items: result.sessions, nextCursor: result.nextCursor }; },
    keyOf: session => session.sessionId,
  });
  // A failed directory replace may retain old readable data, but those sessions
  // must not be offered as members of the newly requested game filter.
  const catalogSessions = catalog.scope === catalogScope ? catalog.items : [];
  const sessions = pinnedSession?.sessionId === sessionFilter && !catalogSessions.some(session => session.sessionId === sessionFilter) ? [pinnedSession, ...catalogSessions] : catalogSessions;
  const locatedUnit = located?.owner === owner && gameFilter === 'all' && sessionFilter === null ? located.unit : null;
  const visibleRecords = locatedUnit && !records.items.some(unit => unitKey(unit) === unitKey(locatedUnit)) ? [locatedUnit, ...records.items] : records.items;
  const selectedBase = visibleRecords.find(unit => unitKey(unit) === selectedKey) ?? visibleRecords[0] ?? null;
  const selected = detail?.owner === owner && selectedBase && unitKey(detail.unit) === unitKey(selectedBase) ? detail.unit : selectedBase;

  useEffect(() => {
    setGameFilter('all'); setSessionFilter(null); setPinnedSession(null); setSelectedKey(null);
    setDetail(null); setLocated(null); setEdit(null); setWord(null); setDetailError(null);
  }, [owner]);

  useEffect(() => {
    if (!active || !selectedBase || editingRef.current) return;
    let current = true;
    setDetailError(null); setDetailLoading(true);
    void store.port.readingUnit(selectedBase.unitId, selectedBase.textRevision).then(unit => { if (current) setDetail({ owner, unit }); })
      .catch(error => { if (current) setDetailError(errorMessage(error)); })
      .finally(() => { if (current) setDetailLoading(false); });
    return () => { current = false; };
  }, [active, owner, store, selectedBase?.unitId, selectedBase?.textRevision, snapshot.commitRevision, detailRefresh]);
  useEffect(() => {
    if (listScroll.current) listScroll.current.scrollTop = 0;
    if (detailScroll.current) detailScroll.current.scrollTop = 0;
  }, [owner, records.scope]);
  useEffect(() => { if (detailScroll.current) detailScroll.current.scrollTop = 0; }, [selectedBase?.unitId, selectedBase?.textRevision]);

  const changeGame = (value: string) => {
    if (editingRef.current) return;
    setGameFilter(value); setSessionFilter(null); setPinnedSession(null); setLocated(null);
  };
  const changeSession = (value: string) => {
    if (editingRef.current) return;
    setSessionFilter(value || null); setPinnedSession(sessions.find(session => session.sessionId === value) ?? null); setLocated(null);
  };
  const refreshRecords = () => { if (!editingRef.current) { setLocated(null); records.refresh(); } };
  const locateCurrent = () => {
    if (editingRef.current || !snapshot.current) return;
    const unit = structuredClone(snapshot.current);
    // Scope changes trigger their own replace. Same-scope queries need an
    // explicit refresh; both requests invalidate their own previous epochs.
    if (gameFilter === 'all') catalog.refresh();
    if (gameFilter === 'all' && sessionFilter === null) records.refresh();
    setGameFilter('all'); setSessionFilter(null); setPinnedSession(null); setLocated({ owner, unit }); setSelectedKey(unitKey(unit));
  };
  const gameLabel = (unit: ReadingUnit) => unit.gameId ? `${snapshot.games.find(game => game.gameId === unit.gameId)?.displayName ?? '已移出游戏'} · ${readingOriginLabel(unit)}` : readingOriginLabel(unit);
  const sessionLabel = (session: HistorySessionSummary) => `${dateLabel(session.startedAtMs)} · ${session.gameId ? snapshot.games.find(game => game.gameId === session.gameId)?.displayName ?? '已移出游戏' : session.sourceKind === 'manual' ? '手动输入' : '未入库游戏'} · ${sessionStateLabel(session.state)}`;
  const selectedSession = selected?.sourceSummary;
  const recordScopeStale = records.previousScope;
  const recordCount = visibleRecords.length;
  return <section className={styles.history} aria-label="历史记录工作区">
    <div className={styles.heading}><PageHeader title="历史记录" description="按游戏与会话回看，继续理解每一句。" actions={<><Button disabled={editing !== null || records.loading} onPress={refreshRecords}>刷新记录</Button><Button icon="search" disabled={!snapshot.current || editing !== null} onPress={locateCurrent}>定位当前句</Button></>}/></div>
    <div className={styles.filters}>
      <Select label="游戏筛选" disabled={editing !== null} value={gameFilter} onChange={changeGame}><option value="all">全部游戏与手动输入</option><option value="manual">手动输入</option>{snapshot.games.map(game => <option key={game.gameId} value={game.gameId}>{game.displayName}</option>)}</Select>
      <div className={styles.sessionTools}>
        <Select label="会话筛选" disabled={editing !== null || sessions.length === 0} value={sessionFilter ?? ''} onChange={changeSession}><option value="">{catalog.loading && !sessions.length ? '正在加载会话…' : !sessions.length ? '暂无会话' : '全部会话'}</option>{sessions.map(session => <option key={session.sessionId} value={session.sessionId}>{sessionLabel(session)}</option>)}</Select>
        <div className={styles.directory}>
          {catalog.failure ? <Button intent="quiet" disabled={editing !== null} onPress={catalog.retry}>重新加载会话</Button> : catalog.canAppend ? <Button intent="quiet" disabled={editing !== null} pending={catalog.loading} onPress={catalog.more}>加载更早会话</Button> : <span className={styles.directoryEnd}>{catalog.loading ? '正在加载会话…' : catalog.initialized && catalog.scope === catalogScope ? '没有更早会话' : '尚无会话'}</span>}
          <small role="status" className={catalog.failure ? styles.error : undefined}>{catalog.failure ? catalog.previousScope ? '新范围会话加载失败，上一目录未用于当前筛选。' : '加载失败 · 已有会话仍可选' : catalog.loading ? '正在读取会话目录…' : `已加载 ${catalogSessions.length} 次会话`}</small>
        </div>
      </div>
    </div>
    {editing ? <p className={styles.editNotice} role="status">正在修改译文。保存或取消后，可切换筛选、选择记录和加载较早内容。</p> : null}
    <div className={styles.layout}>
      <aside className={styles.list} aria-label="历史筛选与记录">
        <header className={styles.listHeading}><strong>{recordScopeStale ? '上一范围的记录' : sessionFilter ? '已选会话' : '全部会话'}</strong><span>{records.loading ? '正在加载…' : `已加载 ${recordCount} 条`}</span></header>
        <div ref={listScroll} className={styles.records} aria-busy={records.loading} aria-label="可滚动的历史记录" tabIndex={0}>
          {visibleRecords.map(unit => <button key={unitKey(unit)} className={`${styles.record} ${selected && unitKey(selected) === unitKey(unit) ? styles.selected : ''}`} disabled={editing !== null} onClick={() => { if (editingRef.current) return; setSelectedKey(unitKey(unit)); setDetail(null); }} aria-pressed={selected ? unitKey(selected) === unitKey(unit) : false}><span lang={unit.sourceLanguage}>{unit.sourceText}</span><small><span>{locatedUnit && unitKey(locatedUnit) === unitKey(unit) ? '当前定位 · ' : ''}{gameLabel(unit)} · 原文 v{unit.textRevision}</span><time dateTime={isoDate(unit.createdAtMs)}>{shortDate(unit.createdAtMs)}</time></small></button>)}
          {!visibleRecords.length ? records.loading ? <p className={styles.loading} role="status">正在读取记录…</p> : records.failure ? <EmptyState title="记录暂时无法读取" description="请重试当前范围，原有保存内容不会被删除。" icon="history"/> : <EmptyState title={sessionFilter ? '这个会话还没有正文' : '这个范围还没有记录'} description={sessionFilter ? '可查看全部会话，或选择其他会话。' : '连接游戏，或手动输入一段文字后，会在这里保留。'} icon="history">{sessionFilter ? <Button disabled={editing !== null} onPress={() => changeSession('')}>查看全部会话</Button> : null}</EmptyState> : null}
        </div>
        <footer className={styles.listFooter}>
          {records.failure ? <Button disabled={editing !== null} onPress={records.retry}>重新加载记录</Button> : records.canAppend ? <Button icon="chevron-down" disabled={editing !== null} pending={records.loading} onPress={records.more}>加载更早记录</Button> : records.initialized && !recordScopeStale && !records.loading ? <span className={styles.ended}><Icon name="check"/>已到最早记录</span> : null}
          <p role={records.failure ? 'alert' : 'status'} className={records.failure ? styles.error : undefined}>{records.failure ? `${records.failure.message} ${recordScopeStale ? '新范围加载失败，仍保留上一范围的记录。' : '加载失败，已显示的记录与选择仍保留。'}` : recordScopeStale ? '正在切换范围，保留上一范围的记录…' : records.loading ? '正在读取记录…' : `已加载 ${recordCount} 条${records.initialized && !records.cursor ? ' · 已显示全部' : ''}`}</p>
        </footer>
      </aside>
      <article ref={detailScroll} className={styles.detail} aria-label="记录详情" tabIndex={0} aria-busy={detailLoading}>
        {selected ? <>
          {action.error ? <Notice error>{action.error}</Notice> : null}
          {detailError ? <Notice error>{detailError} 已保留当前记录。<Button disabled={editing !== null} onPress={() => { if (!editingRef.current) setDetailRefresh(value => value + 1); }}>重新读取详情</Button></Notice> : null}
          <div className={styles.meta}><StatusLabel>{selected.translations.some(translation => translation.correction) ? '人工修正' : selected.sourceSummary?.sourceKind === 'manual' ? '手动翻译' : '阅读记录'}</StatusLabel><time dateTime={isoDate(selected.createdAtMs)}>{dateLabel(selected.createdAtMs)}</time></div>
          <h2>原文</h2>
          <div className={styles.original}><SourceText unit={selected} preferences={snapshot.preferences} lexicalState={snapshot.lexicalStates.find(state => state.unitId === selected.unitId && state.textRevision === selected.textRevision && state.sourceHash === selected.sourceHash)} onRetry={() => void action.run(() => store.port.retryLexical(selected.unitId, selected.textRevision))} onToken={(index, anchor) => setWord({ owner, unit: structuredClone(selected), index, anchor })}/></div>
          <h2>{editing ? '修改译文' : '译文'}</h2>
          {editing ? <CorrectionEditor key={editing.translation.translationId} translation={editing.translation} store={store} onClose={() => { if (editingRef.current !== editing) return; setEdit(null); setDetailRefresh(value => value + 1); }}/> : <>
            <div className={styles.translationResults}><TranslationStack unit={selected} fontPt={snapshot.preferences.translationFontPt}/></div><TranslationTaskStates snapshot={snapshot} unit={selected} store={store} active={active}/>
            <div className={styles.actions}>{selected.translations.map(translation => <Button key={translation.translationId} icon="type" label={`修正 ${translation.providerLabel}`} disabled={records.loading} onPress={() => { if (!editingRef.current) setEdit({ owner, unitKey: unitKey(selected), translation: structuredClone(translation) }); }}>编辑译文…{selected.translations.length > 1 ? ` · ${translation.providerLabel}` : ''}</Button>)}<Button icon="history" label="使用当前配置重译" pending={action.pending} onPress={() => void action.run(async () => { await store.port.retranslate(selected.unitId, selected.textRevision, crypto.randomUUID()); await store.refresh(); })}>重新翻译</Button><Button icon="sparkles" onPress={() => void action.run(() => store.port.freezeLearning(selected.unitId, selected.textRevision, null))}>精读这句</Button><Button icon="copy" onPress={() => void action.run(() => store.port.copyReadingUnit(selected.unitId, selected.textRevision))}>复制原文</Button></div>
          </>}
          <dl className={styles.metadata}><div><dt>游戏 / 来源</dt><dd>{gameLabel(selected)}</dd></div><div><dt>记录时间</dt><dd>{dateLabel(selected.createdAtMs)}</dd></div>{selectedSession ? <div><dt>阅读会话</dt><dd>{dateLabel(selectedSession.startedAtMs)} · {sessionStateLabel(selectedSession.state)}</dd></div> : null}<div><dt>翻译</dt><dd>{selected.translations.length ? selected.translations.map(translation => translation.providerLabel).join('、') : '原文记录'}</dd></div></dl>
          <p className={styles.preservation}>{selected.translations.some(translation => translation.correction) ? '人工修正优先展示，后台译文不会覆盖它。' : '重新翻译会生成独立结果，原文与原记录保留。'} 精读与词卡始终固定在选中的这句。</p>
        </> : <EmptyState title={records.loading ? '正在读取历史' : '选择一条记录'} description={records.loading ? '原文与译文将在加载后显示。' : '从左侧记录中查看完整原文、译文与会话来源。'} icon="history"/>}
      </article>
    </div>
    {word?.owner === owner ? <WordCard key={`${word.unit.unitId}:${word.unit.textRevision}:${word.index}`} unit={word.unit} tokenIndex={word.index} anchor={word.anchor} store={store} onClose={() => setWord(null)}/> : null}
  </section>;
}
