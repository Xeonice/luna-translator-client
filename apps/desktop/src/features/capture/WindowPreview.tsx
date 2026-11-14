import { useEffect, useId, useRef, useState } from 'react';
import type { PreviewCapability, PreviewConsumer, PreviewProjection, PreviewUpdate } from '../../bridge/contracts.generated';
import type { ApplicationPort } from '../../bridge/application-port';
import { errorMessage } from '../../bridge/application-port';
import { Button } from '../../ui/primitives/Controls';
import { Icon } from '../../ui/icons/Icon';
import { PreviewDetails } from './PreviewDetails';
import type { PreviewTargetFacts } from './PreviewDetails';
import styles from './styles.module.css';

const stateLabels: Record<PreviewProjection['state'], string> = { off: '未开启', requesting_permission: '等待授权', starting: '正在开启', live: '实时预览', paused: '已暂停', stale: '画面已过期', minimized: '窗口已最小化', target_closed: '目标窗口已关闭', permission_denied: '需要屏幕录制权限', permission_revoked: '屏幕录制权限已撤销', unavailable: '预览不可用', stopped: '已关闭' };
function timestamp(value: string | null | undefined) { if (!value) return '尚无成功画面'; const date = new Date(Number(value)); return Number.isFinite(date.getTime()) ? date.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false, fractionalSecondDigits: 3 }) : value; }
function timestampIso(value: string | null | undefined) { if (!value) return undefined; const date = new Date(Number(value)); return Number.isFinite(date.getTime()) ? date.toISOString() : undefined; }
const decimal = (value: string) => /^(0|[1-9]\d*)$/.test(value);
const clearsFrame = (state: PreviewProjection['state']) => ['off', 'stopped', 'unavailable', 'target_closed', 'permission_denied', 'permission_revoked'].includes(state);
const selectionErrors = new Set(['preview_window_selection_required', 'preview_selection_required', 'preview_selection_cancelled', 'preview_selection_timeout', 'preview_selection_mismatch', 'preview_selection_unsupported', 'preview_selection_busy', 'preview_selection_expired', 'preview_selection_failed']);

/** Rendering owns only its exact subscription. Explicit controls belong to the owner. */
export function WindowPreview({ port, candidateId, title, detail, identityDetail, targetFacts, onReconnect, consumer = 'candidate', consumerId: requestedConsumerId, candidateProjection, onProjection, onPreviewIntent }: { port: ApplicationPort; candidateId: string | null; title: string; detail: string; identityDetail?: string; targetFacts?: PreviewTargetFacts; onReconnect?: () => void; consumer?: PreviewConsumer; consumerId?: string; candidateProjection?: PreviewProjection | null; onProjection?: (projection: PreviewProjection) => void; onPreviewIntent?: (intent: 'retry' | 'select' | 'pause' | 'resume') => Promise<PreviewProjection> }) {
  const [localConsumerId] = useState(() => crypto.randomUUID());
  const consumerId = requestedConsumerId ?? localConsumerId;
  const targetAvailable = consumer === 'session' ? !!requestedConsumerId : !!candidateId;
  const [capability, setCapability] = useState<PreviewCapability | null>(null);
  const currentCapability = useRef<PreviewCapability | null>(null);
  const [projection, setProjection] = useState<PreviewProjection | null>(null);
  const [image, setImage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const [authorizationOpen, setAuthorizationOpen] = useState(false);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const detailsId = useId();
  const detailsTrigger = useRef<HTMLButtonElement | null>(null);
  const active = useRef<PreviewProjection | null>(null);
  const epoch = useRef(0);
  const frameSequence = useRef<string | null>(null);
  const url = useRef<string | null>(null);
  const displayedFrame = useRef<PreviewUpdate['frame']>(null);
  const clearImage = (resetSequence = true) => { if (url.current) URL.revokeObjectURL(url.current); url.current = null; displayedFrame.current = null; if (resetSequence) frameSequence.current = null; setImage(null); };
  const permissionRecovered = useRef(false);
  const permissionRecoveryPending = useRef(false);
  const readDegraded = useRef(false);
  const recover = useRef<() => Promise<void>>(async () => {});
  const chooseWindow = useRef<() => Promise<void>>(async () => {});
  const callbacks = useRef({ onProjection, onPreviewIntent, candidateProjection });
  callbacks.current = { onProjection, onPreviewIntent, candidateProjection };
  const controls = useRef<(action: 'pause' | 'resume') => Promise<void>>(async () => {});
  const receiveCandidate = useRef<(projection: PreviewProjection) => void>(() => {});
  const apply = (update: PreviewUpdate, requestEpoch: number) => {
    const next = update.projection;
    if (requestEpoch !== epoch.current || next.consumerId !== consumerId || !decimal(next.generation)) return false;
    const previous = active.current;
    const incomingFrame = update.frame;
    const capabilityBlocked = currentCapability.current?.permission === 'denied' || currentCapability.current?.supported === false;
    if (capabilityBlocked && (incomingFrame || !['permission_denied', 'permission_revoked'].includes(next.state))) return false;
    if (next.state === 'live' && !incomingFrame && readDegraded.current && previous?.leaseId === next.leaseId && previous?.generation === next.generation) return false;
    if (incomingFrame && (incomingFrame.leaseId !== next.leaseId || incomingFrame.generation !== next.generation || incomingFrame.targetIdentityHash !== next.targetIdentityHash || incomingFrame.encoding !== 'jpeg' || !decimal(incomingFrame.sequence) || previous?.generation === next.generation && frameSequence.current !== null && BigInt(incomingFrame.sequence) <= BigInt(frameSequence.current))) return false;
    if (previous && (BigInt(next.generation) < BigInt(previous.generation) || next.generation === previous.generation && previous.leaseId && next.leaseId !== previous.leaseId)) return false;
    if (previous?.targetIdentityHash && previous.leaseId === next.leaseId && previous.generation === next.generation && previous.targetIdentityHash !== next.targetIdentityHash) return false;
    if (previous && previous.generation === next.generation && next.state === 'live' && clearsFrame(previous.state)) {
      const frame = update.frame;
      if (!frame || !decimal(frame.sequence) || frameSequence.current !== null && BigInt(frame.sequence) <= BigInt(frameSequence.current)) return false;
      if (['target_closed','stopped','off'].includes(previous.state) || ['preview_selection_expired','preview_selection_mismatch','preview_selection_failed','preview_target_identity','preview_generation_conflict'].includes(previous.errorCode ?? '')) return false;
      if ((previous.state === 'permission_denied' || previous.state === 'permission_revoked') && !permissionRecovered.current) return false;
    }
    if ((next.state === 'permission_denied' || next.state === 'permission_revoked') && previous?.state !== next.state) permissionRecovered.current = false;
    if (previous && (next.generation !== previous.generation || next.leaseId !== previous.leaseId)) {
      if (next.state === 'paused' && next.leaseId === previous.leaseId && next.targetIdentityHash === previous.targetIdentityHash) frameSequence.current = null;
      else clearImage();
    }
    if (previous && next.targetIdentityHash !== previous.targetIdentityHash) clearImage();
    // A cached last frame in a non-live projection is display data, not proof
    // that this subscription has recovered. Only a valid live frame is proof.
    if (next.state !== 'live') readDegraded.current = true;
    else if (incomingFrame) { readDegraded.current = false; permissionRecoveryPending.current = false; }
    active.current = next; setProjection(next); setError(next.errorCode === 'preview_transport' ? next.detail : null);
    callbacks.current.onProjection?.(next);
    if (clearsFrame(next.state) || !next.leaseId) { clearImage(false); return true; }
    const frame = update.frame;
    if (!frame || frame.leaseId !== next.leaseId || frame.generation !== next.generation || frame.targetIdentityHash !== next.targetIdentityHash || frame.encoding !== 'jpeg' || !decimal(frame.sequence) || frameSequence.current !== null && BigInt(frame.sequence) <= BigInt(frameSequence.current)) return true;
    const retained = displayedFrame.current;
    if (next.state === 'paused' && retained && retained.leaseId === frame.leaseId && retained.targetIdentityHash === frame.targetIdentityHash && retained.contentHash === frame.contentHash && retained.capturedAtMs === frame.capturedAtMs && retained.width === frame.width && retained.height === frame.height) { frameSequence.current = frame.sequence; displayedFrame.current = frame; return true; }
    const nextUrl = URL.createObjectURL(new Blob([new Uint8Array(frame.payload)], { type: 'image/jpeg' }));
    if (url.current) URL.revokeObjectURL(url.current);
    url.current = nextUrl; frameSequence.current = frame.sequence; displayedFrame.current = frame; setImage(nextUrl); return true;
  };
  useEffect(() => {
    const requestEpoch = ++epoch.current;
    active.current = null; currentCapability.current = null; permissionRecovered.current = false; permissionRecoveryPending.current = false; readDegraded.current = false; setProjection(null); clearImage(); setCapability(null); setError(null); setAuthorizationOpen(false); setDetailsOpen(false);
    let running = false;
    let subscription: { key: string; unsubscribe: () => void } | null = null;
    const accept = (value: PreviewProjection) => {
      if (requestEpoch !== epoch.current || value.consumerId !== consumerId) return;
      const accepted = apply({ projection: value, frame: null }, requestEpoch);
      const previous = active.current;
      const reattachAfterGrant = permissionRecoveryPending.current && currentCapability.current?.supported === true && currentCapability.current.permission === 'granted' && previous?.leaseId === value.leaseId && previous?.generation === value.generation && previous?.targetIdentityHash === value.targetIdentityHash;
      if (!accepted && !reattachAfterGrant) return;
      const key = JSON.stringify([value.leaseId, value.generation, value.consumerId, value.targetIdentityHash]);
      if (!value.leaseId) { subscription?.unsubscribe(); subscription = null; return; }
      if (subscription?.key === key) return;
      subscription?.unsubscribe();
      const own = { key, unsubscribe: () => {} };
      subscription = own;
      own.unsubscribe = port.attachWindowPreview(value, update => { if (subscription === own) apply(update, requestEpoch); });
    };
    receiveCandidate.current = accept;
    const refresh = async () => {
      if (running || requestEpoch !== epoch.current || !targetAvailable) return;
      running = true; setPending(true);
      try {
        const nextCapability = await port.previewCapability();
        if (requestEpoch !== epoch.current) return;
        const wasBlocked = currentCapability.current?.permission === 'denied' || currentCapability.current?.supported === false;
        currentCapability.current = nextCapability; setCapability(nextCapability);
        if (!nextCapability.supported || nextCapability.permission === 'denied') {
          const previous = active.current, hadFrame = !!displayedFrame.current;
          permissionRecovered.current = false; permissionRecoveryPending.current = true; readDegraded.current = true;
          subscription?.unsubscribe(); subscription = null; clearImage(false); setError(null);
          if (previous) {
            const state = !nextCapability.supported ? 'unavailable' : hadFrame || previous.state === 'permission_revoked' ? 'permission_revoked' : 'permission_denied';
            const deniedProjection: PreviewProjection = { ...previous, state, permission: nextCapability.permission, capturedAtMs: null, contentChangedAtMs: null, errorCode: nextCapability.errorCode, detail: nextCapability.detail };
            active.current = deniedProjection; setProjection(deniedProjection);
          }
        } else if (nextCapability.permission === 'granted') {
          permissionRecovered.current = true;
          // A read begun while denied must not deliver as new grant evidence.
          if (wasBlocked) { subscription?.unsubscribe(); subscription = null; }
        }
        if (consumer === 'session') accept(await port.previewSession());
        else if (permissionRecoveryPending.current && nextCapability.supported && nextCapability.permission === 'granted') {
          const knownCandidate = active.current ?? callbacks.current.candidateProjection;
          if (knownCandidate) accept(knownCandidate);
        }
      } catch (cause) {
        if (requestEpoch === epoch.current) { readDegraded.current = true; setError(errorMessage(cause)); }
      } finally { running = false; if (requestEpoch === epoch.current) setPending(false); }
    };
    const intent = async (kind: 'retry' | 'select' | 'pause' | 'resume') => {
      if (running || requestEpoch !== epoch.current || !targetAvailable) return;
      running = true; setPending(true); setError(null);
      try {
        let result: PreviewProjection;
        if (consumer === 'candidate') {
          if (!callbacks.current.onPreviewIntent) throw new Error('候选预览控制归当前窗口选择，请重新核对目标。');
          result = await callbacks.current.onPreviewIntent(kind);
        } else if (kind === 'select') result = await port.selectWindowPreview({ candidateId: '', consumer, consumerId, expectedGeneration: active.current?.generation ?? null });
        else if (kind === 'retry' && !active.current?.leaseId) result = await port.previewSession();
        else {
          const current = active.current; if (!current?.leaseId) return;
          result = await port.controlWindowPreview({ leaseId: current.leaseId, generation: current.generation, action: kind === 'pause' ? 'pause' : 'resume' });
        }
        if (requestEpoch === epoch.current) accept(result);
      } catch (cause) { if (requestEpoch === epoch.current) { readDegraded.current = true; setError(errorMessage(cause)); } }
      finally { running = false; if (requestEpoch === epoch.current) setPending(false); }
    };
    recover.current = () => intent('retry'); chooseWindow.current = () => intent('select');
    controls.current = action => intent(action);
    void refresh();
    const timer = setInterval(() => { void refresh(); }, 3000);
    return () => {
      epoch.current += 1; clearInterval(timer); subscription?.unsubscribe(); receiveCandidate.current = () => {};
      active.current = null; if (url.current) URL.revokeObjectURL(url.current); url.current = null;
    };
  }, [candidateId, consumerId, consumer, port, targetAvailable]);
  useEffect(() => { if (consumer === 'candidate' && candidateProjection) receiveCandidate.current(candidateProjection); }, [consumer, candidateProjection]);
  const control = (action: 'pause' | 'resume') => controls.current(action);
  const enabled = !!projection?.leaseId && !['off', 'stopped'].includes(projection.state);
  const live = !error && capability?.permission !== 'denied' && capability?.supported !== false && projection?.state === 'live' && !!image;
  const pausedFrame = !error && capability?.permission !== 'denied' && capability?.supported !== false && projection?.state === 'paused' && !!image;
  const denied = projection?.state === 'permission_denied' || projection?.state === 'permission_revoked' || capability?.permission === 'denied';
  const currentState = capability?.permission === 'denied' ? projection?.state === 'permission_revoked' ? 'permission_revoked' : 'permission_denied' : error ? image ? 'stale' : 'unavailable' : capability?.supported === false ? 'unavailable' : projection?.state === 'live' && !image ? 'starting' : projection?.state ?? (denied ? 'permission_denied' : 'starting');
  const selectionRequired = !!projection?.errorCode && selectionErrors.has(projection.errorCode);
  const firstSelection = !denied && ['preview_window_selection_required', 'preview_selection_required'].includes(projection?.errorCode ?? '');
  const stateTitle = firstSelection ? '需要确认游戏窗口' : currentState === 'stale' && !image ? '尚无可用画面' : stateLabels[currentState];
  const guidance = !targetAvailable ? '没有可核验的窗口引用，游戏取文继续。'
    : denied ? '允许 Luna 录屏后自动恢复预览。'
    : firstSelection ? '需要在系统中选择对应的游戏窗口，确认这次窗口共享。此操作只用于预览，游戏取文独立。'
    : projection?.errorCode === 'preview_selection_cancelled' ? '已取消本次窗口共享。需要预览时，请明确重新选择对应的游戏窗口。'
    : projection?.errorCode === 'preview_selection_timeout' ? '系统窗口选择已超时。需要预览时，请明确重新选择对应的游戏窗口。'
    : projection?.errorCode === 'preview_selection_failed' ? '系统未确认这次窗口共享，请重新选择。'
    : selectionRequired ? '重新选择对应的游戏窗口，再核对画面。'
    : currentState === 'paused' ? image ? '已保留最后画面；游戏取文继续。' : '预览已暂停；游戏取文继续。'
    : live ? '自动更新所选窗口的画面。'
    : currentState === 'target_closed' ? onReconnect ? '重新打开游戏后，再选择运行窗口。' : '重新打开游戏后，在上方选择运行窗口。'
    : currentState === 'minimized' ? '恢复游戏窗口，等待画面更新。'
    : currentState === 'stale' ? '画面暂未更新，可刷新重试。'
    : currentState === 'unavailable' ? '恢复游戏窗口，或刷新重试。'
    : '等待有效画面；游戏取文继续。';
  const emptyTitle = firstSelection ? '选择对应的游戏窗口' : currentState === 'starting' ? '正在连接采集器' : currentState === 'target_closed' ? '预览已停止' : currentState === 'unavailable' && !denied ? '窗口当前不可采集' : stateTitle;
  const shownFrame = image && !clearsFrame(currentState) ? displayedFrame.current : null;
  const frameFacts = shownFrame ? { capturedAtMs: shownFrame.capturedAtMs, sequence: shownFrame.sequence, width: shownFrame.width, height: shownFrame.height, encoding: shownFrame.encoding } : null;
  const openDetails = (authorization = false) => { setAuthorizationOpen(authorization); setDetailsOpen(true); };
  return <section className={styles.preview} aria-label="运行窗口预览" data-preview-state={currentState}>
    <div className={styles.previewLayout}>
    <header className={styles.previewHeading}><strong title={title}>{title}</strong><Button buttonRef={detailsTrigger} intent="quiet" icon="ellipsis" label="预览详情" aria-haspopup="dialog" aria-expanded={detailsOpen} aria-controls={detailsId} onPress={() => openDetails()}/></header>
    <div className={styles.previewImage}>
      {image ? <img src={image} className={live ? undefined : styles.staleFrame} alt={`${title} · ${live ? '目标窗口当前画面' : '最后成功画面，当前预览非实时'}`}/> : null}
      {!live && !pausedFrame || !image ? <div className={styles.previewEmpty}><Icon name={['preview_target_identity', 'preview_selection_mismatch'].includes(projection?.errorCode ?? '') ? 'alert' : 'window'} size={23}/><strong>{emptyTitle}</strong></div> : null}
      {image && !live ? <small className={styles.lastFrame}>最后成功画面</small> : null}
      <span className={styles.previewState}>{stateTitle}</span>
    </div>
    <div className={styles.previewMeta}><span><span className={styles.previewTime}><span>{live ? '采集时间' : '原采集时间'}</span><time dateTime={timestampIso(shownFrame?.capturedAtMs)} title={timestampIso(shownFrame?.capturedAtMs) ?? '尚无成功画面'}>{timestamp(shownFrame?.capturedAtMs)}</time></span><small><Icon name={targetAvailable ? 'check' : 'alert'} size={12}/>运行窗口预览 · {targetAvailable ? '已选择目标' : '身份未核对'}</small></span><div className={styles.previewGuidance} role={error ? 'alert' : undefined}>{guidance}</div></div>
    <footer className={styles.previewActions}><Button icon="history" disabled={!targetAvailable || projection?.state === 'target_closed'} pending={pending} onPress={() => void recover.current()}>刷新</Button><Button icon={projection?.state === 'paused' ? 'play' : 'pause'} pressed={projection?.state === 'paused'} disabled={!enabled || !!error || clearsFrame(currentState)} pending={pending} onPress={() => void control(projection?.state === 'paused' ? 'resume' : 'pause')}>{projection?.state === 'paused' ? '继续' : '暂停'}</Button>{denied ? <span className={styles.previewRecovery}><Button onPress={() => openDetails(true)}>授权说明…</Button></span> : selectionRequired ? <span className={styles.previewRecovery}><Button pending={pending} disabled={!targetAvailable || projection?.errorCode === 'preview_selection_unsupported'} onPress={() => void chooseWindow.current()}>{firstSelection ? '选择对应的游戏窗口…' : '重新选择…'}</Button></span> : onReconnect && (currentState === 'target_closed' || currentState === 'stopped' || currentState === 'off') ? <span className={styles.previewRecovery}><Button onPress={onReconnect}>重新关联…</Button></span> : null}</footer>
    </div>
    <PreviewDetails open={detailsOpen} onClose={() => setDetailsOpen(false)} dialogId={detailsId} anchorRef={detailsTrigger} title={title} detail={detail} identityDetail={identityDetail} targetFacts={targetFacts} consumer={consumer} consumerId={consumerId} stateLabel={stateTitle} guidance={guidance} projection={projection} capability={capability} frame={frameFacts} error={error} authorizationOpen={authorizationOpen}/>
  </section>;
}
