import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import type { AppSnapshot, SessionDataset, SessionExportPlan, SessionRecord, SessionRecordFilter, SessionRecordPage, SessionRecordQuery } from '../../bridge/contracts.generated';
import type { ApplicationStore } from '../../bridge/projections';
import { errorMessage } from '../../bridge/application-port';
import { Button, Select, Sheet } from '../../ui/primitives/Controls';
import { EmptyState, Notice } from '../../ui/patterns/Patterns';
import { Icon } from '../../ui/icons/Icon';
import styles from './styles.module.css';

const labels: Record<SessionDataset, string> = { body: '正文历史', extraction: '全部提取', hook_events: 'Hook 日志' };
const emptyFilter = (): SessionRecordFilter => ({ search: null, sourceId: null, level: null, stage: null });
const stageLabels: Record<string, string> = { association: '目标关联', resource: '资源核对', identity: '身份核对', injection: 'Hook 注入', handshake: '宿主握手', stream: '文本流', selection: '正文选择', capture: '接收', disconnect: '断开', recovery: '恢复' };
function completenessText(status: string) {
  return ({ complete_received_range: '本次已接收范围均已保存', incomplete: '本次接收存在已知缺口', unknown: '接收完整性待确认', cleaned: '部分原始载荷已清理' })[status] ?? '接收完整性待确认';
}
function recordTime(record: SessionRecord) { return new Date(record.kind === 'body' ? record.unit.createdAtMs : record.observedAtMs).toLocaleTimeString('zh-CN', { hour12: false }); }

type ReadKind = 'latest' | 'older' | 'refresh';
type RecordLoadError = { message: string; kind: ReadKind; recovery: 'retry' | 'latest' | 'retry_latest' };
function invalidRange(cause: unknown): 'expired' | 'changed' | null {
  if (!cause || typeof cause !== 'object' || !('code' in cause) || !('stage' in cause) || !('recoverable' in cause) || !('message' in cause)
    || cause.stage !== 'session_records' || cause.recoverable !== true || typeof cause.message !== 'string') return null;
  return cause.code === 'snapshot_expired' ? 'expired' : cause.code === 'invalid_cursor' || cause.code === 'cursor_range_changed' ? 'changed' : null;
}
type LoadedPage = { cursor: string; page: SessionRecordPage };
type BrowseAnchor = { identity: string | null; offset: number; top: number; height: number };
// The range freezes membership and source identity, not a unit's later persisted translations.
function recordIdentity(record: SessionRecord) {
  return JSON.stringify(record.kind === 'body'
    ? [record.kind, record.sequence, record.unit.sessionId, record.unit.unitId, record.unit.textRevision, record.unit.sourceHash, record.unit.sourceText, record.sourceRefs]
    : record.kind === 'extraction'
      ? [record.kind, record.sequence, record.eventId, record.sourceId, record.wireSequence]
      : [record.kind, record.sequence, record.eventId, record.sessionId, record.operationId]);
}
function sameRange(left: SessionRecordPage, right: SessionRecordPage) {
  return left.snapshot.snapshotId === right.snapshot.snapshotId && left.snapshot.highWatermark === right.snapshot.highWatermark && left.snapshot.matchingCount === right.snapshot.matchingCount;
}
function browseAnchor(node: HTMLDivElement | null): BrowseAnchor | null {
  if (!node) return null;
  const top = node.getBoundingClientRect().top;
  const row = [...node.querySelectorAll<HTMLElement>('[data-record-sequence]')].find(item => item.getBoundingClientRect().bottom > top);
  return { identity: row ? `${row.dataset.recordKind}:${row.dataset.recordSequence}` : null, offset: row ? row.getBoundingClientRect().top - top : 0, top: node.scrollTop, height: node.scrollHeight };
}

function retainedAnchor(node: HTMLDivElement | null, active: boolean, expanded: boolean, saved: BrowseAnchor | null): BrowseAnchor | null {
  return active && expanded && node && node.clientHeight > 0 && node.clientWidth > 0 ? browseAnchor(node) : saved;
}

export function SessionRecordPanel({ snapshot, store, sessionId, dataset, disclosure = false, active = true }: { snapshot: AppSnapshot; store: ApplicationStore; sessionId: string; dataset: SessionDataset; disclosure?: boolean; active?: boolean }) {
  const [expanded, setExpanded] = useState(!disclosure);
  const expandedRef = useRef(expanded);
  const activeRef = useRef(active);
  const savedScrollTop = useRef(0);
  const savedBrowseAnchor = useRef<BrowseAnchor | null>(null);
  expandedRef.current = expanded; activeRef.current = active;
  const [search, setSearch] = useState('');
  const [source, setSource] = useState('');
  const [level, setLevel] = useState('');
  // The repository applies this view before counting/paging, not to 50 UI rows.
  const [stage, setStage] = useState('diagnostics');
  const [page, setPage] = useState<SessionRecordPage | null>(null);
  const [records, setRecords] = useState<SessionRecord[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<RecordLoadError | null>(null);
  const retryKind = useRef<ReadKind>('latest');
  const [follow, setFollow] = useState(true);
  const [exportOpen, setExportOpen] = useState(false);
  const [exportDataset, setExportDataset] = useState(dataset);
  const [exportScope, setExportScope] = useState<'session' | 'filtered'>('session');
  const [privacyConfirmed, setPrivacyConfirmed] = useState(false);
  const [exportPlan, setExportPlan] = useState<SessionExportPlan | null>(null);
  const [exportBusy, setExportBusy] = useState(false);
  const [exportError, setExportError] = useState<string | null>(null);
  const [exportReceipt, setExportReceipt] = useState<string | null>(null);
  const generation = useRef(0);
  const loadInFlight = useRef<object | null>(null);
  const pendingRead = useRef<{ kind: ReadKind; query: SessionRecordQuery; generation: number } | null>(null);
  const pendingAnchor = useRef<BrowseAnchor | null>(null);
  const pageRef = useRef<SessionRecordPage | null>(null);
  const recordsRef = useRef<SessionRecord[]>([]);
  const loadedPages = useRef<LoadedPage[]>([]);
  const errorRef = useRef<string | null>(null);
  const followRef = useRef(follow);
  const planRef = useRef<SessionExportPlan | null>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const filter = useMemo<SessionRecordFilter>(() => ({ search: search || null, sourceId: dataset === 'extraction' ? source || null : null, level: dataset === 'hook_events' ? level || null : null, stage: dataset === 'hook_events' ? stage || null : null }), [search, source, level, stage, dataset]);
  const query = useMemo<SessionRecordQuery>(() => ({ sessionId, dataset, filter, cursor: null, pageSize: 50, order: 'newest_first' }), [sessionId, dataset, filter]);
  const queryRef = useRef(query);
  queryRef.current = query;
  followRef.current = follow;
  const read = useCallback(async (kind: ReadKind, showBusy = true): Promise<void> => {
    if (loadInFlight.current) {
      // Keep one explicit intent while a read owns the transport. Background polls never queue.
      if (showBusy) {
        const queued = pendingRead.current;
        if (kind === 'latest' || queued?.kind !== 'latest') pendingRead.current = { kind, query, generation: generation.current };
        setBusy(true);
      }
      return;
    }
    const frozen = pageRef.current;
    if (kind !== 'latest' && (!frozen || kind === 'older' && !frozen.nextCursor)) return;
    const current = kind === 'latest' ? ++generation.current : generation.current;
    const owner = {}; loadInFlight.current = owner;
    const isCurrent = () => current === generation.current && queryRef.current === query && loadInFlight.current === owner;
    if (showBusy) setBusy(true);
    errorRef.current = null; setError(null);
    const commitLatest = (result: SessionRecordPage) => {
      loadedPages.current = [{ cursor: result.snapshot.refreshCursor, page: result }];
      pageRef.current = result; recordsRef.current = result.records;
      pendingAnchor.current = null; setPage(result); setRecords(result.records);
    };
    const fail = (cause: unknown, recovery: RecordLoadError['recovery'] = 'retry', message = errorMessage(cause)) => {
      retryKind.current = kind; errorRef.current = message; setError({ message, kind, recovery });
    };
    try {
      if (kind === 'refresh') {
        // Replay every loaded opaque page cursor serially. Commit only a complete valid batch.
        const refreshed: LoadedPage[] = [];
        for (const loaded of loadedPages.current) {
          const result = await store.port.sessionRecords({ ...query, cursor: loaded.cursor });
          if (!isCurrent()) return;
          if (pendingRead.current) return; // Drain explicit navigation before replaying more background pages.
          if (!sameRange(frozen!, result) || JSON.stringify(result.records.map(recordIdentity)) !== JSON.stringify(loaded.page.records.map(recordIdentity))) throw new Error('刷新返回了不同记录或查看范围，已显示记录保留。');
          refreshed.push({ cursor: loaded.cursor, page: result });
        }
        if (!refreshed.length || !isCurrent()) return;
        const first = refreshed[0].page;
        if (followRef.current && first.snapshot.newerCount !== '0') {
          const latestPage = await store.port.sessionRecords({ ...query, cursor: null });
          if (!isCurrent()) return;
          if (followRef.current) {
            commitLatest(latestPage); return;
          }
        }
        pendingAnchor.current = followRef.current ? null : retainedAnchor(listRef.current, activeRef.current, expandedRef.current, savedBrowseAnchor.current);
        loadedPages.current = refreshed;
        const merged = [...refreshed].reverse().flatMap(loaded => loaded.page.records);
        const projection = { ...frozen!, snapshot: first.snapshot, completeness: first.completeness };
        pageRef.current = projection; recordsRef.current = merged;
        setPage(projection); setRecords(merged);
      } else {
        const cursor = kind === 'latest' ? null : frozen!.nextCursor;
        const result = await store.port.sessionRecords({ ...query, cursor });
        if (!isCurrent()) return;
        if (kind === 'older') {
          if (!sameRange(frozen!, result)) throw new Error('分页范围发生变化，已显示记录保留。请返回最新范围后重试。');
          const seen = new Set(recordsRef.current.map(record => `${record.kind}:${record.sequence}`));
          if (result.records.some(record => seen.has(`${record.kind}:${record.sequence}`))) throw new Error('收到重复分页，已显示记录保留。');
          pendingAnchor.current = retainedAnchor(listRef.current, activeRef.current, expandedRef.current, savedBrowseAnchor.current);
          loadedPages.current = [...loadedPages.current, { cursor: cursor!, page: result }];
        } else {
          loadedPages.current = [{ cursor: result.snapshot.refreshCursor, page: result }];
          pendingAnchor.current = null;
        }
        const merged = kind === 'latest' ? result.records : [...result.records, ...recordsRef.current];
        pageRef.current = result; recordsRef.current = merged;
        setPage(result); setRecords(merged);
      }
    } catch (cause) {
      if (!isCurrent()) return;
      const range = invalidRange(cause);
      if (range === 'expired' && kind !== 'latest' && followRef.current && !pendingRead.current) {
        // A frozen range has an absolute lifetime. Replace it only after one successful
        // fresh read, while this exact scope still owns the transport and follows.
        try {
          const latestPage = await store.port.sessionRecords({ ...query, cursor: null });
          if (!isCurrent() || pendingRead.current) return;
          if (followRef.current) commitLatest(latestPage);
          else fail(cause, 'latest');
        } catch (recoveryCause) {
          if (isCurrent() && !pendingRead.current) fail(recoveryCause, 'retry_latest', `${errorMessage(cause)} 获取最新范围失败：${errorMessage(recoveryCause)}`);
        }
      } else fail(cause, range ? 'latest' : 'retry');
    } finally {
      if (isCurrent()) {
        loadInFlight.current = null; setBusy(false);
        const queued = pendingRead.current; pendingRead.current = null;
        if (queued && queued.query === query && queued.generation === current) void read(queued.kind);
      }
    }
  }, [query, store]);
  useEffect(() => {
    generation.current += 1; loadInFlight.current = null; pendingRead.current = null;
    pageRef.current = null; recordsRef.current = []; loadedPages.current = []; pendingAnchor.current = null;
    savedScrollTop.current = 0; savedBrowseAnchor.current = null;
    errorRef.current = null; setPage(null); setRecords([]); setError(null);
    void read('latest');
    return () => { generation.current += 1; loadInFlight.current = null; pendingRead.current = null; };
  }, [read]);
  useEffect(() => {
    const timer = setInterval(() => { if (activeRef.current && expandedRef.current && pageRef.current && errorRef.current === null) void read('refresh', false); }, 1000);
    return () => clearInterval(timer);
  }, [read]);
  useEffect(() => () => { const plan = planRef.current; if (plan) void store.port.cancelSessionExport(plan.exportId).catch(() => {}); }, [store]);
  useLayoutEffect(() => {
    const node = listRef.current;
    // A mounted, expanded panel can still be hidden by main-page navigation.
    // Keep its pending anchor until a visible layout exists again.
    if (!node || !active || !expanded || node.clientHeight === 0 || node.clientWidth === 0) return;
    const anchor = pendingAnchor.current;
    if (anchor) {
      const row = [...node.querySelectorAll<HTMLElement>('[data-record-sequence]')].find(item => `${item.dataset.recordKind}:${item.dataset.recordSequence}` === anchor.identity);
      node.scrollTop = row ? node.scrollTop + row.getBoundingClientRect().top - node.getBoundingClientRect().top - anchor.offset : anchor.top + node.scrollHeight - anchor.height;
      pendingAnchor.current = null;
    } else if (followRef.current) node.scrollTop = node.scrollHeight;
    else node.scrollTop = savedScrollTop.current;
    savedScrollTop.current = node.scrollTop; savedBrowseAnchor.current = browseAnchor(node);
  }, [records, expanded, active]);
  const load = async (latest = false) => { await read(latest ? 'latest' : 'older'); };
  const retry = async () => { await read(retryKind.current); };
  const latest = () => { followRef.current = true; setFollow(true); void load(true); };
  const pauseFollow = () => { if (followRef.current) { followRef.current = false; setFollow(false); } };
  const closeExport = async () => { if (exportBusy) return; try { if (planRef.current) await store.port.cancelSessionExport(planRef.current.exportId); planRef.current = null; setExportPlan(null); setExportOpen(false); } catch (cause) { setExportError(errorMessage(cause)); } };
  const prepareExport = async () => {
    setExportBusy(true); setExportError(null);
    const current = generation.current;
    try {
      const sameFilter = exportScope === 'filtered' && exportDataset === dataset;
      const exportQuery: SessionRecordQuery = { sessionId, dataset: exportDataset, filter: exportScope === 'filtered' ? { ...filter, sourceId: exportDataset === 'extraction' ? filter.sourceId : null, level: exportDataset === 'hook_events' ? filter.level : null, stage: exportDataset === 'hook_events' ? filter.stage : null } : emptyFilter(), cursor: sameFilter ? page?.snapshot.refreshCursor ?? null : null, pageSize: 50, order: 'newest_first' };
      const result = await store.port.prepareSessionExport({ query: exportQuery, privacyConfirmed });
      if (current !== generation.current) { await store.port.cancelSessionExport(result.exportId); return; }
      planRef.current = result; setExportPlan(result);
    } catch (cause) { setExportError(errorMessage(cause)); }
    finally { setExportBusy(false); }
  };
  const confirmExport = async () => {
    if (!exportPlan) return; setExportBusy(true); setExportError(null);
    try { const result = await store.port.confirmSessionExport(exportPlan.exportId); planRef.current = null; setExportPlan(null); setExportOpen(false); setExportReceipt(result.status === 'completed' ? `已导出 ${result.recordCount} 条记录 · ${result.destinationLabel ?? '所选文件'}` : '已取消保存，当前会话记录保留。'); }
    catch (cause) { setExportError(errorMessage(cause)); }
    finally { setExportBusy(false); }
  };
  const filtered = Boolean(search || source || level || dataset === 'hook_events' && stage);
  const loadedRange = { first: records[0]?.sequence ?? '—', last: records.at(-1)?.sequence ?? '—' };
  const panel = <section className={`${styles.sessionPanel} ${dataset === 'body' ? styles.bodyHistory : ''}`} aria-label={labels[dataset]} data-session-dataset={dataset}>
    <div className={styles.recordIntro}><div><h2>{dataset === 'hook_events' ? 'Hook 接入日志' : dataset === 'extraction' ? '本次会话的全部提取文本' : '本次会话正文历史'}</h2><p>{dataset === 'hook_events' ? stage === 'diagnostics' ? '显示真实 Core 诊断与接入结果；原始回调的保存通知可在“全部事件”查看，原文在“全部提取”保留。' : '从目标核对、接入到恢复的全部事件，按发生顺序保留。' : dataset === 'extraction' ? '所有已接收来源均可查看；未选中的流不进入阅读字幕。' : '按接收顺序保留每次正文；完整原始内容及其他来源在“全部提取”中查看。'}</p></div><span>{`共 ${page?.snapshot.matchingCount ?? '—'} 条${filtered ? '匹配记录' : ''}`}</span></div>
    <div className={styles.recordToolbar}>
      <label className={styles.recordSearch}><Icon name="search" size={13}/><input aria-label={dataset === 'hook_events' ? '搜索 Hook 日志' : '搜索会话原文'} value={search} onChange={event => { pauseFollow(); setSearch(event.target.value); }} placeholder={dataset === 'hook_events' ? '搜索日志' : '搜索原文'}/></label>
      {dataset === 'extraction' ? <Select label="文本来源" value={source} onChange={value => { pauseFollow(); setSource(value); }}><option value="">全部文本来源</option>{snapshot.session?.sources.map(item => <option key={item.sourceId} value={item.sourceId}>{item.hookName || item.sourceId} · {item.selected ? '已选正文' : '未选中'}</option>)}</Select> : null}
      {dataset === 'hook_events' ? <><Select label="日志级别" value={level} onChange={value => { pauseFollow(); setLevel(value); }}><option value="">所有级别</option><option value="info">常规事件</option><option value="warning">警告</option><option value="error">错误</option></Select><Select label="事件范围" value={stage} onChange={value => { pauseFollow(); setStage(value); }}><option value="diagnostics">接入诊断</option><option value="">全部事件</option>{Object.entries(stageLabels).map(([value, label]) => <option value={value} key={value}>{label}</option>)}</Select></> : null}
      <Button icon={follow ? 'check' : 'pause'} pressed={follow} onPress={() => follow ? pauseFollow() : latest()}>{follow ? '跟随新内容' : `回到最新${page?.snapshot.newerCount && page.snapshot.newerCount !== '0' ? ` · ${page.snapshot.newerCount} 条新内容` : ''}`}</Button>
      <Button onPress={() => { setExportDataset(dataset); setExportScope('session'); setPrivacyConfirmed(false); setExportError(null); setExportOpen(true); }}>导出…</Button>
    </div>
    {page ? <div className={styles.recordRange} role="status" aria-live="off"><div><span>{completenessText(page.completeness.status)}</span><span>本次查看范围固定，晚到内容单独提示</span>{page.completeness.cleanedPayloadCount !== '0' ? <span>已清理 {page.completeness.cleanedPayloadCount} 条载荷，记录位置保留</span> : null}</div><details onToggle={event => { if (event.currentTarget.open) pauseFollow(); }}><summary>查看保存范围与接收详情</summary><dl>{[
      ['本次查看截至', page.snapshot.highWatermark], ['最早可用记录', page.snapshot.firstAvailable], ['最后收到', page.snapshot.lastReceived], ['最后保存', page.snapshot.lastPersisted], ['保留起点', page.snapshot.retainedFrom], ['已加载范围', `${loadedRange.first}～${loadedRange.last}`], ['完整性适用范围', '仅已接收到客户端的记录；不代表上游从未丢失'], ['缺失数量', page.completeness.unknownRange ? '未知' : page.completeness.knownLoss ? '详见关联 Hook 事件' : '未检测到已知缺失'], ...page.completeness.notes.map(note => ['范围说明', note]),
    ].map(([label, value], index) => <div key={`${label}:${index}`}><dt>{label}</dt><dd>{value ?? '—'}</dd></div>)}</dl></details></div> : null}
    {page && (page.completeness.status === 'incomplete' || page.completeness.status === 'unknown') ? <Notice error={page.completeness.knownLoss}><strong>{completenessText(page.completeness.status)}</strong><p>已保存内容继续保留。无法确认的缺失不会补造为完整文本。</p></Notice> : null}
    {exportReceipt ? <Notice>{exportReceipt}</Notice> : null}
    <div ref={listRef} className={styles.records} role="log" data-platform-scrollbar aria-label={dataset === 'hook_events' ? 'Hook 事件列表' : dataset === 'body' ? '正文历史列表' : '全量提取文本列表'} aria-live="off" data-session-id={sessionId} data-high-watermark={page?.snapshot.highWatermark} onWheel={event => { if (event.deltaY < 0) pauseFollow(); }} onScroll={() => { const node = listRef.current; if (!node || !activeRef.current || !expandedRef.current || node.clientHeight === 0 || node.clientWidth === 0) return; savedScrollTop.current = node.scrollTop; savedBrowseAnchor.current = browseAnchor(node); if (node.scrollHeight - node.clientHeight - node.scrollTop > 32) pauseFollow(); }}>
      {records.length ? records.map(record => <RecordRow key={`${record.kind}:${record.sequence}`} record={record} sessionId={sessionId} snapshot={snapshot} onBrowse={pauseFollow}/>) : <EmptyState title={busy ? '正在加载会话记录' : filtered ? '没有匹配结果' : '等待收到会话内容'} description={busy ? '读取当前固定范围，原文接收继续。' : search ? '尝试其他关键词或清除筛选。' : '收到后会按数据集的稳定顺序出现在这里。'}/>}
    </div>
    <footer className={styles.recordFooter}><span>{`已显示 ${records.length} / ${page?.snapshot.matchingCount ?? '—'} 条${search ? '匹配记录' : ''} · 记录 ${loadedRange.first}～${loadedRange.last}`}{page && page.snapshot.newerCount !== '0' ? <><br/>新到 {page.snapshot.newerCount} 条，当前查看位置保持不变</> : null}</span>{error ? <><span className={styles.recordLoadError} role="alert">{error.recovery === 'latest' ? '本次查看范围已失效，已显示内容和位置保留。请回到最新范围继续。' : error.recovery === 'retry_latest' ? '获取最新范围失败，已显示内容保留。可重试获取最新范围。' : `${error.kind === 'older' ? '加载较早记录' : error.kind === 'refresh' ? '刷新会话记录' : '加载最新记录'}失败，已显示内容保留。`}<details><summary>错误详情</summary>{error.message}</details></span><Button pending={busy} onPress={() => error.recovery === 'retry' ? void retry() : latest()}>{error.recovery === 'latest' ? '回到最新范围' : error.recovery === 'retry_latest' ? '重试获取最新范围' : '重试'}</Button></> : page?.nextCursor ? <Button pending={busy} onPress={() => { pauseFollow(); void load(); }}>加载较早记录</Button> : page ? <span>已到记录起点</span> : null}</footer>
    <Sheet title="导出当前会话" description="文件可能包含游戏原文及程序身份。确认内容和范围后，再选择系统保存位置。" open={exportOpen} onClose={() => { void closeExport(); }} footer={<><Button disabled={exportBusy} onPress={() => { void closeExport(); }}>取消</Button>{exportPlan ? <Button intent="primary" pending={exportBusy} onPress={() => void confirmExport()}>选择保存位置并导出</Button> : <Button intent="primary" disabled={!privacyConfirmed} pending={exportBusy} onPress={() => void prepareExport()}>核对导出范围</Button>}</>}>
      {exportError ? <Notice error>{exportError} 会话数据库和已显示记录保持不变。</Notice> : null}
      {exportPlan ? <dl className={styles.recordDetails}><div><dt>内容</dt><dd>{labels[exportPlan.dataset]}</dd></div><div><dt>固定范围</dt><dd>{exportPlan.snapshot.firstAvailable ?? '—'}–{exportPlan.snapshot.highWatermark}</dd></div><div><dt>匹配记录</dt><dd>{exportPlan.snapshot.matchingCount} 条</dd></div><div><dt>文件顺序</dt><dd>{exportPlan.order === 'oldest_first' ? '从最早到最新' : '从最新到最早'}</dd></div><div><dt>接收完整性</dt><dd>{exportPlan.completeness.status} · 仅已接收范围</dd></div><div><dt>隐私内容</dt><dd>{exportPlan.containsGameText ? '包含游戏原文。' : ''}{exportPlan.containsTargetIdentity ? '包含目标和程序身份。' : ''}不包含预览帧或密钥。</dd></div></dl> : <div className={styles.exportFields}><Select label="导出内容范围" value={exportDataset} onChange={value => setExportDataset(value as SessionDataset)}><option value="body">正文历史</option><option value="extraction">全部提取</option><option value="hook_events">Hook 日志</option></Select><Select label="导出记录范围" value={exportScope} onChange={value => setExportScope(value as 'session' | 'filtered')}><option value="session">完整会话</option><option value="filtered">当前过滤结果</option></Select><label><input type="checkbox" checked={privacyConfirmed} onChange={event => setPrivacyConfirmed(event.target.checked)}/>我已了解导出可能包含完整台词和程序身份，并确认保存与分享范围。</label></div>}
    </Sheet>
  </section>;
  if (!disclosure) return panel;
  const sectionKey = dataset === 'body' ? 'reading' : dataset === 'extraction' ? 'texts' : 'logs';
  const description = dataset === 'body' ? '本次会话的原文与译文' : dataset === 'extraction' ? '含选流前、未选来源与合并前原文' : '接入阶段与警告，按需查看原始事实';
  const loadedWarnings = records.filter(record => record.kind === 'hook_event' && (record.level === 'warning' || record.level === 'error')).length;
  const summaryCount = page ? `上次读取 · ${page.snapshot.matchingCount} 条${filtered ? '匹配记录' : '记录'}` : error ? '读取失败' : busy ? '正在读取…' : '尚未读取';
  const changeExpanded = (next: boolean) => {
    if (next === expandedRef.current) return;
    const node = listRef.current;
    if (!next && activeRef.current && node && node.clientHeight > 0 && node.clientWidth > 0) {
      savedScrollTop.current = node.scrollTop; savedBrowseAnchor.current = browseAnchor(node);
    }
    expandedRef.current = next; setExpanded(next);
    if (next && activeRef.current && pageRef.current && errorRef.current === null) void read('refresh', false);
  };
  return <details className={styles.recordDisclosure} data-flat-session-section={sectionKey} open={expanded}
    onToggle={event => {
      // A nested row's disclosure never owns this panel. A controlled update
      // may also emit toggle later; changeExpanded deduplicates that event.
      if (event.target === event.currentTarget) changeExpanded(event.currentTarget.open);
    }}>
    <summary onClick={event => {
      // Update React ownership in the click/keyboard activation turn. Waiting
      // for native toggle lets an unrelated snapshot render restore old open.
      event.preventDefault(); changeExpanded(!expandedRef.current);
    }}>
      <span className={styles.disclosureDescription}><strong>{dataset === 'hook_events' ? 'Hook 诊断' : labels[dataset]}</strong><small>{description}</small></span>
      <span className={styles.disclosureCount}>{summaryCount}{dataset === 'hook_events' && loadedWarnings > 0 ? <small>已加载中 {loadedWarnings} 条警告或错误</small> : null}{page && (page.completeness.knownLoss || page.completeness.unknownRange || page.completeness.status !== 'complete_received_range') ? <small>{completenessText(page.completeness.status)}</small> : null}</span>
      <span className={styles.disclosureAction}>{expanded ? '收起' : '查看'}<Icon name="chevron-down" size={12}/></span>
    </summary>
    {panel}
  </details>;
}

function RecordRow({ record, sessionId, snapshot, onBrowse }: { record: SessionRecord; sessionId: string; snapshot: AppSnapshot; onBrowse: () => void }) {
  const source = record.kind === 'extraction' ? snapshot.session?.sources.find(item => item.sourceId === record.sourceId) : null;
  const details: [string, string | number | null][] = record.kind === 'hook_event' ? [
    ['事件代码', record.code], ['事件身份', record.eventId], ['会话', record.sessionId ?? '会话建立前'], ['接入操作', record.operationId], ['关联来源', record.sourceId], ['上游序号', record.wireSequence], ['缺失数量', record.lostCount ?? (record.integrity === 'unknown' ? '未知' : '未报告')], ['接收完整性', record.integrity],
  ] : record.kind === 'extraction' ? [
    ['事件身份', record.eventId], ['会话', sessionId], ['文本流', record.sourceId], ['上游序号', record.wireSequence], ['Hook 编码', source?.hookCode ?? null], ['正文关联', record.bodyRefs.map(ref => `${ref.unitId} · v${ref.textRevision}`).join('；') || '未形成正文'], ['载荷保留', record.payloadRetained ? '已保留原始回调文本' : '已清理，不能从正文反造原始载荷'],
  ] : [
    ['会话', record.unit.sessionId], ['阅读单元', record.unit.unitId], ['原文版本', record.unit.textRevision], ['原文身份', record.unit.sourceHash], ...record.sourceRefs.map((ref, index): [string, string] => [`来源 ${index + 1}`, `${ref.eventId} · UTF-8 字节 ${ref.byteStart}～${ref.byteEnd}`]),
  ];
  return <article className={record.kind === 'hook_event' ? styles.logEntry : styles.textEntry} data-record-sequence={record.sequence} data-record-kind={record.kind}>
    <span className={styles.sequence}>#{record.sequence}</span><div>
      <header className={styles.recordMeta}><strong>{record.kind === 'hook_event' ? stageLabels[record.stage] ?? record.stage : record.kind === 'extraction' ? source?.hookName ?? record.sourceId : '正文'}</strong><span className={record.kind === 'hook_event' && record.level !== 'info' ? styles.warningPill : undefined}>{record.kind === 'hook_event' ? record.level === 'error' ? '错误' : record.level === 'warning' ? '警告' : '信息' : record.kind === 'extraction' ? record.reason : `原文 v${record.unit.textRevision}`}</span>{record.kind === 'extraction' ? <span className={styles.association}>{record.bodyRefs.length ? `已形成 ${record.bodyRefs.length} 条正文关联` : '未进入字幕'}</span> : null}<time>{recordTime(record)}</time></header>
      <p lang={record.kind === 'body' ? record.unit.sourceLanguage : undefined}>{record.kind === 'hook_event' ? record.message : record.kind === 'extraction' ? record.payloadRetained ? record.text ?? '没有可显示的原文' : <span className={styles.retainedNotice}>原始载荷已清理。事件位置与正文关联仍保留。</span> : record.unit.sourceText}</p>
      {record.kind === 'body' ? record.unit.translations.length ? record.unit.translations.map(translation => <p className={styles.historyTranslation} key={translation.translationId}>{translation.correction?.text ?? translation.text}<small>{` · ${translation.providerLabel}`}</small></p>) : <p className={styles.historyTranslation}>尚无译文，原文保持可读。</p> : null}
      <details onToggle={event => { if (event.currentTarget.open) onBrowse(); }}><summary>{record.kind === 'hook_event' ? '查看事件详情' : record.kind === 'body' ? '查看正文来源' : '查看完整来源'}</summary><dl className={styles.recordDetails}>{details.map(([label, value], index) => <div key={`${label}:${index}`}><dt>{label}</dt><dd>{value ?? '—'}</dd></div>)}</dl></details>
    </div>
  </article>;
}
