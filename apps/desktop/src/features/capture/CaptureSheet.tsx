import { useCallback, useEffect, useRef, useState } from 'react';
import type * as Contracts from '../../bridge/contracts.generated';
import type { AppSnapshot, Game, GameInput, PreviewProjection, TargetCandidate } from '../../bridge/contracts.generated';
import type { ApplicationStore } from '../../bridge/projections';
import { useAction } from '../../bridge/projections';
import { errorMessage } from '../../bridge/application-port';
import { Button, Select, Sheet, TextField } from '../../ui/primitives/Controls';
import { EmptyState, Notice, StatusLabel } from '../../ui/patterns/Patterns';
import { Icon } from '../../ui/icons/Icon';
import { WindowPreview } from './WindowPreview';
import styles from './styles.module.css';

// Conservative presentation hints only. Rust normalizes and verifies the full target before injection.
const pathHint = (path: string) => path.replaceAll('/', '\\').toLowerCase();
function targetConflict(game: Game, candidate: TargetCandidate) {
  if (game.target.environmentKind === 'crossover' && game.target.environmentKey && game.target.environmentKey !== candidate.environmentKey) return '运行环境或 Bottle 与游戏资料不一致，请选择正确的游戏资料或新建记录。';
  const path = game.target.canonicalExePath;
  if (path && pathHint(path) !== pathHint(candidate.executablePath)) return '完整程序路径与游戏资料不一致，请核对实际游戏，不能把其他程序关联到这款游戏。';
  return null;
}
function candidateInput(candidate: TargetCandidate, displayName: string, language: string): GameInput {
  return { gameId: null, rowRevision: null, displayName: displayName.trim(), originalTitle: null, sourceLanguage: language || null, coverAssetRef: null, target: { environmentKind: 'crossover', environmentKey: candidate.environmentKey, bottleLocator: candidate.environmentKey.startsWith('crossover:') ? candidate.environmentKey.slice('crossover:'.length) : null, launchPath: candidate.executablePath, canonicalExePath: candidate.executablePath, targetPathKey: null, workingDirectory: null, arguments: [], architecture: candidate.architecture, verificationState: 'unresolved' } };
}

type CandidatePreviewOwner = { candidateId: string; consumerId: string; projection: PreviewProjection | null; retired: boolean; stopping?: Promise<void> };
type JourneyOperation = { id: string; context: string; sessionId: string | null };
const contextKey = (snapshot: AppSnapshot) => JSON.stringify([snapshot.profileId, snapshot.serverInstanceId, snapshot.profileGeneration]);
// A retained stopped session supplies the scoped game, not an active-session precondition.
const observedActiveSessionId = (snapshot: AppSnapshot) => snapshot.session && ['opening', 'active'].includes(snapshot.session.state) ? snapshot.session.sessionId : null;
const profileMatches = (profile: Contracts.BindingProfileContext, snapshot: AppSnapshot) => profile.profileId === snapshot.profileId && profile.serverInstanceId === snapshot.serverInstanceId && profile.profileGeneration === snapshot.profileGeneration;
const stagePosition: Record<Contracts.AssociationStage, number | null> = { verifying: 0, preparing_resources: 1, requesting_hook: 1, waiting_handshake: 1, waiting_text: 2, candidates_ready: 3, failed: null, superseded: null };
const progressLabels = ['窗口关联已核验', '自动启动 Hook 接入', '已连接，等待游戏文本', '已有候选，选择正文'];
const stageLabels: Record<Contracts.AssociationStage, string> = { verifying: '核对运行身份', preparing_resources: '准备 Hook 资源', requesting_hook: '已发出 Hook 启动请求', waiting_handshake: '等待 Hook 握手', waiting_text: '已连接，等待游戏文本', candidates_ready: '已有候选，选择正文', failed: '取文接入失败', superseded: '已被后续会话取代' };
function SourceRow({ source, checked, disabled, onChange }: { source: Contracts.SourceDetailView; checked: boolean; disabled: boolean; onChange: (checked: boolean) => void }) {
  return <tr data-source-id={source.sourceId} data-selected={checked}>
    <td><label className={styles.sourceChoice}><input type="checkbox" value={source.sourceId} aria-label={`选择文本流 ${source.hookName || source.sourceId}`} checked={checked} disabled={disabled} onChange={event => onChange(event.target.checked)}/></label></td>
    <td><div className={styles.sourceSample}>{source.latestSample || '等待该流的原文'}</div><div className={styles.sourceName}>{source.hookName || '未命名文本流'}</div>
      <details className={styles.sourceDisclosure}><summary>完整最新样本与来源</summary><p className={styles.fullSample}>{source.latestSample || '尚未收到样本'}</p>
        <details><summary>高级来源信息</summary><dl className={styles.sourceIdentity}>{[['Hook', source.hookName], ...(source.hookCode ? [['Hook代码', source.hookCode]] : []), ['Address', source.address], ['Context', source.context], ['Context 2', source.context2], ['编码', source.encoding]].map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value ?? '未提供'}</dd></div>)}</dl></details>
      </details>
    </td><td><StatusLabel>候选</StatusLabel></td>
  </tr>;
}

/** Interaction drafts only. Rust owns target relations, operation stages and committed selections. */
export function CaptureSheet({ game, snapshot, store, open, onClose, mode = 'target', onRead, attributionIntent = game ? 'explicit_game' : 'auto_match', scopedGameId = game?.gameId ?? null }: { game: Game | null; snapshot: AppSnapshot; store: ApplicationStore; open: boolean; onClose: () => void; mode?: 'target' | 'streams'; onRead: () => void; attributionIntent?: Contracts.AssociationAttributionIntent; scopedGameId?: string | null }) {
  const [candidates, setCandidates] = useState<TargetCandidate[]>([]);
  const [selected, setSelected] = useState<TargetCandidate | null>(null);
  const [step, setStep] = useState<'select' | 'connecting' | 'streams'>(mode === 'streams' ? 'streams' : 'select');
  const [list, setList] = useState<'windows' | 'processes'>('windows');
  const [filter, setFilter] = useState('');
  const [discoveryMessages, setDiscoveryMessages] = useState<string[]>([]);
  const [discovery, setDiscovery] = useState<'idle' | 'scanning' | 'ready' | 'empty' | 'error'>('idle');
  const [mapping, setMapping] = useState(attributionIntent === 'explicit_game' && scopedGameId ? `game:${scopedGameId}` : attributionIntent === 'session_only' ? 'session' : '');
  const mappingEdited = useRef(false);
  const [newName, setNewName] = useState('');
  const [newLanguage, setNewLanguage] = useState('');
  const [savedNotice, setSavedNotice] = useState<string | null>(null);
  const saved = useRef<{ key: string; gameId: string; itemId: string; rowRevision: number } | null>(null);
  const newItemId = useRef(crypto.randomUUID());
  const [previewProjection, setPreviewProjection] = useState<PreviewProjection | null>(null);
  const [previewConsumerId, setPreviewConsumerId] = useState('');
  const previewOwner = useRef<CandidatePreviewOwner | null>(null);
  const [review, setReview] = useState<Contracts.AssociationReview | null>(null);
  const [reviewError, setReviewError] = useState<string | null>(null);
  const [reviewing, setReviewing] = useState(false);
  const [reviewVersion, setReviewVersion] = useState(0);
  const [operation, setOperation] = useState<JourneyOperation | null>(null);
  const operationRef = useRef<JourneyOperation | null>(null);
  const [progress, setProgress] = useState<Contracts.AssociationProgress | null>(null);
  const [progressError, setProgressError] = useState<string | null>(null);
  const [associationFailure, setAssociationFailure] = useState<{ operationId: string | null; review: Contracts.AssociationReview; message: string } | null>(null);
  const retryingAssociation = useRef(false);
  const [selection, setSelection] = useState<Contracts.SourceSelectionProjection | null>(null);
  const [selectionError, setSelectionError] = useState<string | null>(null);
  const [selectionLoading, setSelectionLoading] = useState(false);
  const [sourceIds, setSourceIds] = useState<string[]>([]);
  const [remember, setRemember] = useState(false);
  const [committed, setCommitted] = useState<Contracts.BeginReadingResult | null>(null);
  const submitRequest = useRef<Contracts.BeginReadingRequest | null>(null);
  const sourceEpoch = useRef(0);
  const progressGeneration = useRef<{ operationId: string; generation: string; sequence: string | null } | null>(null);
  const selectionDraft = useRef<{ sessionId: string | null; edited: boolean }>({ sessionId: null, edited: false });
  const action = useAction();
  const submitting = useRef(false);
  const discoveryEpoch = useRef(0);
  const discoveryBusy = useRef(false);
  const reviewEpoch = useRef(0);
  const alive = useRef(true);
  const current = useRef({ open, snapshot, candidateId: selected?.candidateId, context: contextKey(snapshot) });
  current.current = { open, snapshot, candidateId: selected?.candidateId, context: contextKey(snapshot) };
  const context = current.current.context;
  const stopCandidate = useCallback((owner: CandidatePreviewOwner | null) => {
    if (!owner) return Promise.resolve();
    owner.retired = true;
    if (owner.stopping) return owner.stopping;
    const projection = owner.projection;
    owner.stopping = projection?.leaseId && projection.consumerId === owner.consumerId
      ? store.port.controlWindowPreview({ leaseId: projection.leaseId, generation: projection.generation, action: 'stop' }).then(() => {}, () => {})
      : Promise.resolve();
    return owner.stopping;
  }, [store]);
  const cancelPreview = () => { const owner = previewOwner.current; previewOwner.current = null; setPreviewProjection(null); void stopCandidate(owner); };
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false; reviewEpoch.current++; discoveryEpoch.current++; sourceEpoch.current++;
      // A committed association continues in Rust. Its candidate lease transfers under the native fence.
      if (!operationRef.current) void stopCandidate(previewOwner.current);
    };
  }, [stopCandidate]);
  const receivePreview = (projection: PreviewProjection) => {
    const owner = previewOwner.current;
    if (!owner || owner.retired || projection.consumerId !== owner.consumerId) return;
    owner.projection = projection;
    if (alive.current && current.current.open) setPreviewProjection(projection);
  };
  const previewIntent = async (kind: 'retry' | 'select' | 'pause' | 'resume'): Promise<PreviewProjection> => {
    const owner = previewOwner.current;
    if (!owner || owner.retired) throw new Error('候选目标已更换，请重新核对。');
    const previous = owner.projection;
    const request = { candidateId: owner.candidateId, consumer: 'candidate' as const, consumerId: owner.consumerId, expectedGeneration: previous?.generation ?? null };
    // Refresh is not permission to resume a cancelled/expired system selection.
    // Only the explicit select action may reopen the picker and obtain a new filter.
    if (kind === 'retry' && previous?.errorCode && ['preview_window_selection_required', 'preview_selection_required', 'preview_selection_cancelled', 'preview_selection_timeout', 'preview_selection_mismatch', 'preview_selection_expired', 'preview_selection_failed', 'preview_selection_unsupported', 'preview_selection_busy'].includes(previous.errorCode)) return previous;
    const result = kind === 'select' ? await store.port.selectWindowPreview(request)
      : previous?.leaseId ? await store.port.controlWindowPreview({ leaseId: previous.leaseId, generation: previous.generation, action: kind === 'pause' ? 'pause' : 'resume' })
      : await store.port.startWindowPreview({ ...request, expectedGeneration: null });
    if (previewOwner.current !== owner || owner.retired) {
      if (result.consumerId === owner.consumerId && result.leaseId) await store.port.controlWindowPreview({ leaseId: result.leaseId, generation: result.generation, action: 'stop' }).catch(() => {});
    } else receivePreview(result);
    return result;
  };
  const discover = async (pick = false) => {
    if (discoveryBusy.current || submitting.current) return;
    discoveryBusy.current = true;
    const epoch = ++discoveryEpoch.current;
    setDiscovery('scanning'); setDiscoveryMessages([]); action.clearError();
    try {
      const result = await (pick ? store.port.pickWindow(mapping.startsWith('game:') ? mapping.slice(5) : null) : store.port.targets());
      if (!alive.current || epoch !== discoveryEpoch.current) return;
      setCandidates(result.candidates); setDiscoveryMessages(result.diagnostics.map(item => item.message));
      setDiscovery(result.candidates.length ? 'ready' : 'empty');
      if (previewOwner.current && !result.candidates.some(item => item.candidateId === previewOwner.current?.candidateId)) { cancelPreview(); setSelected(null); setReview(null); }
      if (pick && result.candidates.length === 1) choose(result.candidates[0]);
    } catch (cause) {
      if (alive.current && epoch === discoveryEpoch.current) { setDiscovery('error'); setDiscoveryMessages([errorMessage(cause)]); }
    } finally { if (epoch === discoveryEpoch.current) discoveryBusy.current = false; }
  };
  useEffect(() => {
    if (!open || mode === 'streams') return;
    void discover();
    return () => { discoveryEpoch.current++; discoveryBusy.current = false; };
    // Discovery is an observer scoped to this dialog and profile, not to every snapshot publication.
  }, [open, mode, store, context]);
  const visible = candidates.filter(candidate => (list === 'processes' || candidate.hasWindow) && `${candidate.windowTitle} ${candidate.executablePath} ${candidate.bottleLabel}`.toLocaleLowerCase().includes(filter.toLocaleLowerCase()));
  const mappedGame = mapping.startsWith('game:') ? snapshot.games.find(item => item.gameId === mapping.slice(5)) ?? null : null;
  const conflict = selected && mappedGame ? targetConflict(mappedGame, selected) : mapping.startsWith('game:') && !mappedGame ? '游戏资料已不存在，请重新选择归属。' : null;
  const requestedAttribution = mappingEdited.current ? mapping.startsWith('game:') ? 'explicit_game' : 'session_only' : attributionIntent;
  const intendedGameId = requestedAttribution === 'explicit_game' ? mapping.startsWith('game:') ? mapping.slice(5) : scopedGameId : null;
  const readReview = useCallback(async (candidateId: string, gameId: string | null, expectedActiveSessionId: string | null, intent: Contracts.AssociationAttributionIntent, priorReviewId?: string) => {
    const epoch = ++reviewEpoch.current, expectedContext = current.current.context;
    const isCurrentReview = () => {
      const stored = store.getSnapshot().snapshot;
      return alive.current && current.current.open && epoch === reviewEpoch.current && expectedContext === current.current.context
        && (!stored || contextKey(stored) === expectedContext) && current.current.candidateId === candidateId;
    };
    setReviewing(true); setReview(null); setReviewError(null);
    try {
      const result = await store.port.reviewAssociation({ candidateId, gameId, expectedProfileGeneration: current.current.snapshot.profileGeneration, expectedActiveSessionId, attributionIntent: intent, ...(priorReviewId ? { priorReviewId } : {}) });
      if (!isCurrentReview()) return null;
      if (!profileMatches(result.profile, current.current.snapshot) || result.candidate.candidateId !== candidateId) throw new Error('目标核对上下文已变化，请重新核对。');
      setReview(result);
      if (intent === 'auto_match' && !mappingEdited.current) setMapping(result.gameId ? `game:${result.gameId}` : result.relation === 'unavailable' ? '' : result.relation === 'reuse' && current.current.snapshot.session?.sessionId === result.activeSessionId && current.current.snapshot.session.gameId === null ? 'session' : 'new');
      return result;
    } catch (cause) { if (isCurrentReview()) setReviewError(errorMessage(cause)); return null; }
    finally { if (isCurrentReview()) setReviewing(false); }
  }, [store]);
  useEffect(() => {
    if (!open || submitting.current || step !== 'select' || associationFailure || !selected || (!mapping && requestedAttribution !== 'auto_match')) return;
    void readReview(selected.candidateId, intendedGameId, observedActiveSessionId(current.current.snapshot), requestedAttribution);
    return () => { reviewEpoch.current++; };
  }, [open, step, associationFailure, selected?.candidateId, intendedGameId, requestedAttribution, context, reviewVersion, readReview]);
  const refreshProgress = useRef<() => void>(() => {});
  useEffect(() => {
    if (!open || !operation || operation.context !== context) return;
    let live = true, querying = false;
    const update = async () => {
      if (querying || !live) return;
      querying = true;
      try {
        const value = await store.port.associationProgress({ operationId: operation.id, expectedProfileGeneration: current.current.snapshot.profileGeneration });
        if (!live || !alive.current || operation.context !== current.current.context || value.operationId !== operation.id || !profileMatches(value.profile, current.current.snapshot)) return;
        const accepted = progressGeneration.current;
        const sequence = value.facts.at(-1)?.sequence ?? null;
        if (accepted && accepted.operationId === value.operationId && (accepted.generation !== value.generation || accepted.sequence !== null && (sequence === null || BigInt(accepted.sequence) > BigInt(sequence)))) return;
        progressGeneration.current = { operationId: value.operationId, generation: value.generation, sequence };
        setProgress(value);
        setProgressError(null);
        if (value.sessionId) operationRef.current = { ...operation, sessionId: value.sessionId };
        if (value.stage === 'candidates_ready' && !value.superseded) setStep('streams');
      } catch (cause) {
        const code = typeof cause === 'object' && cause !== null && 'code' in cause ? cause.code : null;
        if (live && alive.current && !(submitting.current && code === 'association_operation_missing')) setProgressError(`接入阶段读取未完成：${errorMessage(cause)}`);
      }
      finally { querying = false; }
    };
    refreshProgress.current = () => { void update(); };
    void update();
    // Polling reads actual Rust facts; elapsed time never advances the displayed stage.
    const timer = setInterval(() => void update(), 250);
    return () => { live = false; clearInterval(timer); refreshProgress.current = () => {}; };
  }, [open, operation, context, store]);
  useEffect(() => { refreshProgress.current(); }, [snapshot.revision]);
  const sessionId = operation?.context === context ? progress?.sessionId ?? operation.sessionId : mode === 'streams' ? snapshot.session?.sessionId ?? null : null;
  const sessionCurrent = !!sessionId && snapshot.session?.sessionId === sessionId;
  const querySelection = useCallback(async () => {
    if (!sessionId || !current.current.open) return;
    const expectedContext = current.current.context, expectedSessionId = sessionId, epoch = ++sourceEpoch.current;
    setSelectionLoading(true);
    try {
      const value = await store.port.sourceSelection({ sessionId, expectedProfileGeneration: current.current.snapshot.profileGeneration });
      if (!alive.current || epoch !== sourceEpoch.current || !current.current.open || expectedContext !== current.current.context || current.current.snapshot.session?.sessionId !== expectedSessionId) return;
      if (value.sessionId !== expectedSessionId || !profileMatches(value.profile, current.current.snapshot)) throw new Error('正文来源上下文已变化，请重新核对。');
      setSelection(value); setSelectionError(null);
      if (selectionDraft.current.sessionId !== value.sessionId) { selectionDraft.current = { sessionId: value.sessionId, edited: false }; setRemember(false); setCommitted(null); submitRequest.current = null; }
      if (!selectionDraft.current.edited) setSourceIds(value.state === 'requires_review' ? [] : value.current?.members.map(member => member.sourceId) ?? []);
    } catch (cause) { if (alive.current && epoch === sourceEpoch.current && expectedContext === current.current.context) setSelectionError(errorMessage(cause)); }
    finally { if (alive.current && epoch === sourceEpoch.current && expectedContext === current.current.context) setSelectionLoading(false); }
  }, [sessionId, store]);
  useEffect(() => { if (open && step === 'streams') void querySelection(); }, [open, step, context, snapshot.revision, querySelection]);
  const choose = (candidate: TargetCandidate) => {
    if (submitting.current) return;
    const previous = previewOwner.current;
    const owner: CandidatePreviewOwner = { candidateId: candidate.candidateId, consumerId: crypto.randomUUID(), projection: null, retired: false };
    previewOwner.current = owner; setPreviewConsumerId(owner.consumerId); setPreviewProjection(null);
    setSelected(candidate); setAssociationFailure(null); setStep('select'); setReview(null); setProgress(null); setSelection(null); setSourceIds([]); setCommitted(null); setOperation(null); operationRef.current = null;
    action.clearError(); setSavedNotice(null);
    void stopCandidate(previous).then(async () => {
      if (previewOwner.current !== owner || owner.retired) return;
      try { await previewIntent('retry'); }
      catch (cause) { if (previewOwner.current === owner && !owner.retired) receivePreview({ leaseId: null, generation: '0', consumerId: owner.consumerId, state: 'unavailable', permission: 'unavailable', targetIdentityHash: null, attemptedAtMs: null, capturedAtMs: null, contentChangedAtMs: null, permissionCheckedAtMs: null, errorCode: 'preview_transport', detail: errorMessage(cause) }); }
    });
    mappingEdited.current = false;
    setMapping(attributionIntent === 'explicit_game' && scopedGameId ? `game:${scopedGameId}` : attributionIntent === 'session_only' ? 'session' : '');
    setNewName(candidate.windowTitle || candidate.executablePath.split(/[\\/]/).at(-1)?.replace(/\.exe$/i, '') || ''); setNewLanguage('');
    newItemId.current = crypto.randomUUID(); saved.current = null;
  };
  const associate = (approvedReview: Contracts.AssociationReview | null) => {
    if (!selected || !approvedReview || approvedReview.relation === 'unavailable' || !mapping || conflict || mapping === 'new' && !newName.trim() || submitting.current || retryingAssociation.current || discoveryBusy.current) return;
    const candidate = selected, choice = mapping, expectedContext = context;
    const draft = candidateInput(candidate, newName, newLanguage), key = JSON.stringify([candidate.candidateId, draft]), itemId = newItemId.current;
    let submittedReview = approvedReview;
    submitting.current = true; setAssociationFailure(null);
    void action.run(async () => {
      try {
        let authorized = approvedReview;
        if (choice === 'new') {
          let gameId = saved.current?.key === key ? saved.current.gameId : null;
          if (!gameId) {
            const receipt = await store.command({ kind: 'save_games', payload: { items: [{ itemId, game: draft }] } });
            const results = receipt.gameResults ?? [];
            if (results.length !== 1 || results[0].itemId !== itemId || !results[0].gameId || results[0].rowRevision < 1) throw new Error('保存结果无法对应本次游戏资料，未发起 Hook 接入。请核对后重试。');
            gameId = results[0].gameId;
            saved.current = { key, gameId, itemId, rowRevision: results[0].rowRevision };
          }
          if (alive.current) setSavedNotice('游戏资料已保存；如接入失败，重试将复用这条记录。');
          const updated = await readReview(candidate.candidateId, gameId, approvedReview.activeSessionId, 'explicit_game', approvedReview.reviewId);
          if (!updated || updated.relation === 'unavailable' || updated.relation === 'switch' && approvedReview.relation !== 'switch') throw new Error('保存后目标或当前会话已变化，请重新核对切换影响。资料已保留，不会重复建立。');
          authorized = updated;
        }
        submittedReview = authorized;
        if (expectedContext !== current.current.context) throw new Error('配置档已变化，请重新核对目标。');
        const nextOperation: JourneyOperation = { id: crypto.randomUUID(), context: expectedContext, sessionId: null };
        operationRef.current = nextOperation;
        if (alive.current) { setOperation(nextOperation); setProgress(null); setProgressError(null); setStep('connecting'); }
        const receipt = await store.port.associate({ operationId: nextOperation.id, reviewId: authorized.reviewId });
        if (!profileMatches(receipt.profile, current.current.snapshot) || receipt.operationId !== nextOperation.id || expectedContext !== current.current.context) return;
        const owner = previewOwner.current;
        if (owner?.candidateId === candidate.candidateId) { owner.retired = true; previewOwner.current = null; }
        operationRef.current = { ...nextOperation, sessionId: receipt.sessionId };
        await store.refresh();
        if (alive.current && expectedContext === current.current.context) { setOperation(operationRef.current); refreshProgress.current(); }
      } catch (cause) {
        const isCurrentAttempt = () => {
          const stored = store.getSnapshot().snapshot;
          return alive.current && current.current.open && expectedContext === current.current.context
            && (!stored || contextKey(stored) === expectedContext) && current.current.candidateId === candidate.candidateId;
        };
        // A late failure belongs only to the dialog/profile that issued it.
        if (!isCurrentAttempt()) return;
        setAssociationFailure({ operationId: operationRef.current?.id ?? null, review: submittedReview, message: errorMessage(cause) });
        setStep('select'); setReview(null); operationRef.current = null;
        if (saved.current?.key === key) setMapping(`game:${saved.current.gameId}`);
        const code = typeof cause === 'object' && cause !== null && 'code' in cause ? cause.code : null;
        if (code === 'association_review_stale') {
          // Core kept the later session. Re-read that actual snapshot, then issue
          // only a new review; its switch/owner impact needs another confirmation.
          await store.refresh();
          if (!isCurrentAttempt()) return;
          const refreshed = store.getSnapshot();
          if (refreshed.snapshot && !refreshed.error) {
            const savedGameId = saved.current?.key === key ? saved.current.gameId : null;
            await readReview(candidate.candidateId, savedGameId ?? intendedGameId,
              observedActiveSessionId(refreshed.snapshot), savedGameId ? 'explicit_game' : requestedAttribution);
            if (!isCurrentAttempt()) return;
          }
        }
        throw cause;
      }
    }).finally(() => { submitting.current = false; });
  };
  const changeSource = (sourceId: string, checked: boolean) => {
    selectionDraft.current.edited = true; submitRequest.current = null;
    setSourceIds(previous => checked ? previous.includes(sourceId) ? previous : [...previous, sourceId] : previous.filter(id => id !== sourceId));
    action.clearError();
  };
  const completeWindowEffects = (result: Contracts.BeginReadingResult) => {
    setCommitted(result);
    if (result.effects.showHud.state === 'succeeded' && result.effects.hideMain.state === 'succeeded') { onRead(); onClose(); }
  };
  const beginReading = () => {
    if (!selection || !sessionCurrent || !sourceIds.length || sourceIds.length > selection.memberLimit || submitting.current || committed) return;
    const expectedContext = context;
    const request = submitRequest.current ?? { operationId: crypto.randomUUID(), sessionId: selection.sessionId, expectedProfileGeneration: snapshot.profileGeneration, expectedSetRevision: selection.current?.setRevision ?? null, sourceIds: [...sourceIds], remember: remember && selection.rememberAllowed };
    submitRequest.current = request; submitting.current = true;
    void action.run(async () => {
      const result = await store.port.beginReading(request);
      await store.refresh();
      if (!alive.current || expectedContext !== current.current.context || current.current.snapshot.session?.sessionId !== request.sessionId) return;
      if (result.selection.operationId !== request.operationId || result.selection.selection.sessionId !== request.sessionId || !profileMatches(result.selection.profile, current.current.snapshot) || result.effects.authority.operationId !== result.selection.effectAuthority.operationId || result.effects.authority.setRevision !== result.selection.selection.setRevision) throw new Error('收到的阅读回执不属于本次提交，请重新同步后核对。');
      completeWindowEffects(result);
    }).finally(() => { submitting.current = false; });
  };
  const retryWindowEffects = () => {
    if (!committed || submitting.current) return;
    const receipt = committed, expectedContext = context;
    submitting.current = true;
    void action.run(async () => {
      const effects = await store.port.retryReadingWindowEffects(receipt.selection.effectAuthority);
      if (alive.current && expectedContext === current.current.context && current.current.snapshot.session?.sessionId === receipt.selection.selection.sessionId && effects.authority.operationId === receipt.selection.operationId && effects.authority.setRevision === receipt.selection.selection.setRevision) completeWindowEffects({ selection: receipt.selection, effects });
    }).finally(() => { submitting.current = false; });
  };
  const close = () => { if (submitting.current && !operationRef.current) return; if (!operationRef.current) cancelPreview(); onClose(); };
  const busy = action.pending || discovery === 'scanning';
  const canAssociate = !!selected && !!mapping && !conflict && !reviewing && !!review && review.relation !== 'unavailable' && profileMatches(review.profile, snapshot) && (mapping !== 'new' || !!newName.trim());
  const relation = review?.relation;
  const stageIndex = progress ? stagePosition[progress.stage] : null;
  const failedProgress = associationFailure?.operationId && progress?.operationId === associationFailure.operationId ? progress : null;
  const lastKnownFailureStage = failedProgress ? [...failedProgress.facts].reverse().find(fact => fact.stage !== 'failed' && fact.stage !== 'superseded')?.stage : undefined;
  const progressFailed = progress?.stage === 'failed' || progress?.stage === 'superseded' || progress?.superseded;
  const selectionUnavailable = !sessionCurrent || !selection || selection.sessionId !== sessionId || !profileMatches(selection.profile, snapshot);
  const confirmLabel = relation === 'switch' ? '切换并接入' : relation === 'reuse' ? '复用连接并继续' : '关联并接入';
  const retryAssociation = async () => {
    if (!selected || submitting.current || retryingAssociation.current) return;
    retryingAssociation.current = true; action.clearError(); setStep('select');
    try {
      const fresh = await readReview(selected.candidateId, intendedGameId, observedActiveSessionId(current.current.snapshot), requestedAttribution);
      if (!fresh || fresh.relation === 'unavailable') return;
      // A fresh switch review must be presented before the user confirms its impact.
      // A retry of none/reuse cannot silently authorize stopping a later target.
      if (fresh.relation === 'switch' || fresh.gameId !== (associationFailure?.review.gameId ?? review?.gameId ?? null)) return;
      retryingAssociation.current = false;
      associate(fresh);
    } finally { retryingAssociation.current = false; }
  };
  return <Sheet size="wide" appearance={step === 'select' ? 'library' : 'default'} title={step === 'streams' ? '选择要显示的文本' : step === 'connecting' ? progressFailed ? '窗口已关联，取文接入未完成' : '正在接入游戏' : '选择正在运行的游戏'} description={step === 'streams' ? '让原文与画面的正文一致。' : step === 'connecting' ? '已确认目标；自动完成身份核验与 Hook 接入。' : '选择窗口或进程，核对环境、完整路径与本次运行身份。确认关联时同步启动 Hook 接入。'} open={open} onClose={close} footer={step === 'streams' ? <><Button onPress={close}>稍后选择</Button><small className={styles.selectionCount}>已选择 {sourceIds.length} 项</small>{committed ? <Button intent="primary" pending={action.pending} disabled={!committed.effects.retryable} onPress={retryWindowEffects}>重试窗口操作</Button> : <Button intent="primary" pending={action.pending} disabled={selectionUnavailable || !sourceIds.length || sourceIds.length > (selection?.memberLimit ?? 0)} onPress={beginReading}>开始阅读</Button>}</> : step === 'connecting' ? <Button onPress={close}>关闭面板</Button> : <><Button disabled={action.pending} onPress={close}>取消</Button><Button intent="primary" pending={action.pending} disabled={!canAssociate || discovery === 'scanning'} onPress={() => associate(review)}>{confirmLabel}</Button></>}>
    {step !== 'select' ? <ol className={styles.journeySteps} aria-label="接入流程">{['窗口已关联', '自动接入', '选择正文'].map((label, index) => <li key={label} aria-current={index === (step === 'streams' ? 2 : 1) ? 'step' : undefined}><b>{index + 1}</b>{label}</li>)}</ol> : null}
    {action.error && !associationFailure ? <Notice error>{action.error} 目标与草稿仍保留，可以复核后重试。</Notice> : null}
    {savedNotice ? <Notice>{savedNotice}</Notice> : null}
    {associationFailure && step === 'select' ? <section className={styles.switchReview} aria-label="上次接入失败">
      <strong>上次接入未完成</strong><p>{associationFailure.message}</p>
      {failedProgress ? <p>实际阶段：{stageLabels[failedProgress.stage]}{lastKnownFailureStage ? `；最近阶段：${stageLabels[lastKnownFailureStage]}` : ''}</p> : null}
      <details className={styles.identity}><summary>接入操作详情</summary><dl><div><dt>操作</dt><dd>{associationFailure.operationId ?? '尚未发起 Hook 接入'}</dd></div><div><dt>最近事实</dt><dd>{progress?.operationId === associationFailure.operationId ? progress.facts.at(-1)?.message ?? '尚无可读取的阶段事实' : '没有属于本次操作的阶段回执'}</dd></div></dl></details>
      <div className={styles.retryActions}><Button pending={reviewing || action.pending} onPress={() => void retryAssociation()}>重试接入</Button></div>
    </section> : null}

    {step === 'select' ? <div className={styles.targetFields} role="group" aria-label="运行目标核对" aria-busy={busy}>
      <div className={styles.targetColumn}><div className={styles.listControls}><div role="group" aria-label="目标列表"><button type="button" aria-pressed={list === 'windows'} onClick={() => setList('windows')}>运行窗口</button><button type="button" aria-pressed={list === 'processes'} onClick={() => setList('processes')}>进程列表</button></div><Button icon="scan" disabled={busy} onPress={() => void discover(true)}>点击选择窗口…</Button></div>
      <div className={styles.filter}><TextField label="筛选窗口、程序或 Bottle" value={filter} onChange={setFilter} disabled={busy} placeholder="搜索本机运行目标"/><Button icon="scan" disabled={busy} pending={discovery === 'scanning'} onPress={() => void discover()}>扫描进程</Button></div>
      {discovery === 'scanning' ? <p className={styles.discoveryStatus} role="status">正在扫描运行中的游戏…扫描完成后可选择目标。</p> : null}
      {discovery === 'error' ? <Notice error>{discoveryMessages.join('；')}</Notice> : discoveryMessages.length ? <details className={styles.secondaryWarnings}><summary>扫描说明与其他容器（{discoveryMessages.length}）</summary>{discoveryMessages.map((message, index) => <p key={index}>{message}</p>)}</details> : null}
      <div className={styles.candidates} role="radiogroup" aria-label="实际取文目标">{visible.map(candidate => <label className={`${styles.candidate} ${selected?.candidateId === candidate.candidateId ? styles.selected : ''}`} key={candidate.candidateId}><input type="radio" name="target" disabled={busy} checked={selected?.candidateId === candidate.candidateId} onChange={() => choose(candidate)}/><span className={styles.targetIcon}><Icon name={candidate.hasWindow ? 'window' : 'activity'} size={21}/></span><span className={styles.targetContent}><strong>{candidate.windowTitle || candidate.executablePath.split(/[\\/]/).at(-1) || '无标题进程'}</strong><small>{candidate.bottleLabel} · {candidate.architecture} · Windows PID {candidate.windowsPid}</small></span><span className={styles.targetCheck}>{selected?.candidateId === candidate.candidateId ? <Icon name="check"/> : null}</span></label>)}</div>
      {!visible.length && discovery !== 'scanning' ? <EmptyState title={discovery === 'error' ? '扫描失败，可以重试' : candidates.length ? '没有匹配的目标' : discovery === 'empty' ? '没有发现运行中的游戏' : '等待扫描运行中的游戏'} description="先启动游戏，再扫描本机窗口与进程。Steam 与启动器通常不是实际取文目标。" icon="window"/> : null}
      {selected ? <section className={styles.mapping} aria-label="关联与实际取文目标"><h3>关联并接入的目标</h3>
        <Select label="关联到游戏库" value={mapping} disabled={busy} onChange={value => { mappingEdited.current = true; setAssociationFailure(null); setMapping(value); setReview(null); action.clearError(); }} placeholder={requestedAttribution === 'auto_match' && !mapping && reviewing ? '正在核对游戏库归属…' : '请选择游戏资料'} options={[...snapshot.games.map(item => ({ value: `game:${item.gameId}`, label: item.displayName })), { value: 'new', label: '新建游戏资料并关联' }, { value: 'session', label: '仅本次连接，不加入游戏库' }]}/>
        {conflict ? <Notice error>{conflict}</Notice> : null}
        {mapping === 'session' ? <p className={styles.note}>只建立本次取文会话，不保存游戏资料或游戏库封面。</p> : null}
        {mapping === 'new' ? <div className={styles.newRecord}><TextField label="游戏名称" value={newName} onChange={setNewName} required disabled={busy}/><Select label="原文语言" value={newLanguage} onChange={setNewLanguage} disabled={busy} options={[{ value: '', label: '尚未确认' }, { value: 'ja', label: '日语' }, { value: 'zh-CN', label: '中文' }, { value: 'en', label: '英语' }]}/><p className={styles.note}>确认后保存这份资料并立即接入；语言不确定时保留未确认，不按游戏名推断。</p></div> : null}
        <dl><div><dt>游戏资料</dt><dd>{mappedGame?.displayName ?? (mapping === 'new' ? newName || '待填写' : mapping === 'session' ? '仅本次连接' : '尚未选择')}</dd></div><div><dt>窗口</dt><dd>{selected.windowTitle || '没有可用标题'}</dd></div><div><dt>Windows 游戏进程</dt><dd>{`PID ${selected.windowsPid} · ${selected.executablePath.split(/[\\/]/).at(-1)}`}</dd></div><div><dt>环境与容器</dt><dd>CrossOver · {selected.bottleLabel}</dd></div><div><dt>目标完整路径</dt><dd>{selected.executablePath}</dd></div><div><dt>程序架构</dt><dd>{selected.architecture}</dd></div></dl>
        <details className={styles.identity}><summary>高级运行身份</summary><dl><div><dt>Windows HWND</dt><dd>{selected.windowHandle ?? '未提供'}</dd></div><div><dt>本次进程创建身份</dt><dd>{selected.processCreatedU64}</dd></div><div><dt>macOS宿主PID</dt><dd>{selected.macosHostPid ?? '未提供'}</dd></div><div><dt>macOS窗口身份</dt><dd>{selected.macosWindowId ?? '未提供'}</dd></div></dl></details>
      </section> : null}</div>
      {selected ? <WindowPreview key={selected.candidateId} port={store.port} candidateId={selected.candidateId} consumerId={previewConsumerId} candidateProjection={previewProjection} onProjection={receivePreview} onPreviewIntent={previewIntent} title={selected.windowTitle || '所选运行目标'} detail={`${selected.bottleLabel} · ${selected.executablePath.split(/[\\/]/).at(-1)}`} identityDetail={`${selected.candidateId} · PID ${selected.windowsPid}`} targetFacts={{ ...selected, origin: 'candidate' }}/> : null}
      {reviewing ? <p className={styles.note} role="status">正在核对完整运行身份与当前会话…</p> : null}
      {reviewError ? <><Notice error>{reviewError}</Notice><Button onPress={() => setReviewVersion(value => value + 1)}>重新核对目标</Button></> : null}
      {review ? relation === 'switch' ? <section className={styles.switchReview} role="note"><strong>将切换当前取文目标</strong><p>当前：{snapshot.games.find(item => item.gameId === snapshot.session?.gameId)?.displayName ?? '当前会话'}；切换到：{selected?.windowTitle || selected?.executablePath}</p><p>{review.detail}</p><p>确认后停止核对时的旧连接，再接入所选目标。取消保留当前会话与历史。</p></section> : <p className={styles.note} role={relation === 'unavailable' ? 'alert' : 'status'}>{relation === 'reuse' ? '完整目标与本次运行身份一致，将复用当前 Hook，不重复注入。' : null}{review.detail}</p> : null}
      <p className={styles.note}>关联成功、Hook 连接、收到候选与选择正文分别显示。预览不可用时，已核对目标仍可独立取文。</p>
    </div> : step === 'connecting' ? <>
      <div className={styles.associatedTarget}><Icon name="window"/><div><strong>{selected?.windowTitle || selected?.executablePath || '已确认运行目标'}</strong><small>{selected?.bottleLabel} · {selected?.architecture}</small></div><StatusLabel>{progress ? stageLabels[progress.stage] : '正在接入'}</StatusLabel></div>
      <ol className={styles.progress} aria-label="自动接入阶段">{progressLabels.map((label, index) => <li key={label} className={stageIndex === index ? styles.current : stageIndex !== null && stageIndex > index ? styles.complete : ''} aria-current={stageIndex === index ? 'step' : undefined}><span>{stageIndex !== null && stageIndex > index ? <Icon name="check"/> : index + 1}</span>{label}</li>)}</ol>
      {progressError ? <Notice error>{progressError}</Notice> : null}
      {progressFailed ? <><Notice error>{progress?.error?.message ?? '当前接入已失效，请重新核对运行目标后重试。'}</Notice><div className={styles.retryActions}><Button intent="primary" onPress={() => void retryAssociation()}>重试接入</Button></div></> : <p className={styles.note}>关联确认后自动接入，无需再次点击连接。关闭面板不会停止已提交的接入操作。</p>}
    </> : <>
      {committed ? <Notice>选择已保存，窗口操作未完成。{committed.effects.detail} 可重试窗口操作，无需再次提交正文选择。</Notice> : null}
      {!sessionCurrent ? <Notice error>当前会话已变化，请关闭面板并重新核对正文来源。草稿尚未提交。</Notice> : null}
      {selectionError ? <><Notice error>正文来源读取未完成：{selectionError}</Notice><div className={styles.retryActions}><Button pending={selectionLoading} onPress={() => void querySelection()}>重试读取来源</Button></div></> : null}
      {selectionLoading && !selection ? <p className={styles.sourceStatus} role="status">正在读取真实正文来源…</p> : null}
      {selection?.saved ? <section className={styles.savedSelection}><strong>上次为这款游戏保存的选择</strong><p>{selection.saved.state === 'requires_review' || selection.state === 'requires_review' ? '需要重新核对。' : null}{selection.saved.reason}</p><p>{selection.saved.sources.map(source => source.hookName || source.sourceId).join('、')}</p></section> : null}
      <p className={styles.sourceStatus}>{selection?.sources.length ? `${sessionCurrent && ['connected', 'active'].includes(snapshot.session?.state ?? '') ? '已连接采集端 · ' : ''}${selection.sources.length} 个候选流。比较内容后选择正文，可选多项。` : '在游戏中推进一段对白，等待真实候选文字。不会自动选择第一条。'}</p>
      {selection?.sources.length ? <table className={styles.sourceTable}><thead><tr><th scope="col">显示</th><th scope="col">文字预览</th><th scope="col">来源</th></tr></thead><tbody>{selection.sources.map(source => <SourceRow key={source.sourceId} source={source} checked={sourceIds.includes(source.sourceId)} disabled={action.pending || !!committed || selectionUnavailable || !sourceIds.includes(source.sourceId) && sourceIds.length >= selection.memberLimit} onChange={checked => changeSource(source.sourceId, checked)}/>)}</tbody></table> : !selectionLoading && !selectionError ? <EmptyState title="等待真实候选" description="保持游戏运行并推进对白。收到的来源将保留完整样本与高级身份，勾选后再明确开始阅读。" icon="activity"/> : null}
      <div className={styles.remember}><label className={styles.sourceChoice}><input type="checkbox" checked={remember} disabled={action.pending || !!committed || !selection?.rememberAllowed || !sessionCurrent} onChange={event => { setRemember(event.target.checked); submitRequest.current = null; }}/>为这款游戏记住选择</label><p className={styles.note}>{selection?.rememberAllowed ? '可取消勾选，临时使用本次选择。' : '当前没有可保存选择的游戏归属，仅可临时使用本次正文来源。'}不会只凭 Hook 名称合并不同文本流。{selection?.saved ? '临时选择不会覆盖上次保存的说明。' : null}</p></div>
    </>}
  </Sheet>;
}
