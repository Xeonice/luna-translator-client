import type { PreviewFrame, PreviewProjection, PreviewUpdate } from './contracts.generated';

type ReadLatest = (leaseId: string, generation: string, afterSequence: string | null) => Promise<PreviewUpdate>;
type Subscription = { listener: (update: PreviewUpdate) => void; sequence: string | null; targetIdentityHash: string | null };
type TransportNotice = { state: 'stale' | 'permission_revoked' | 'target_closed' | 'unavailable'; denied: boolean; code: string };
type PreviewPull = {
  nativeProjection: PreviewProjection;
  lastFrame: PreviewFrame | null;
  transportNotice?: TransportNotice;
  observed: boolean;
  active: boolean;
  sequence: string | null;
  pending?: Promise<PreviewUpdate>;
  timer?: ReturnType<typeof setTimeout>;
  listeners: Map<symbol, Subscription>;
};

const sameOwner = (left: PreviewProjection, right: PreviewProjection) => left.leaseId === right.leaseId && left.generation === right.generation && left.consumerId === right.consumerId;
const sameContext = (left: PreviewProjection, right: PreviewProjection) => sameOwner(left, right) && left.targetIdentityHash === right.targetIdentityHash;
const clearsFrame = (projection: PreviewProjection) => ['off', 'stopped', 'unavailable', 'target_closed', 'permission_denied', 'permission_revoked'].includes(projection.state);

/** Each WebView owns transport subscriptions; capture truth remains in Rust. */
export class PreviewSubscriptionClient {
  private readonly pulls = new Map<string, PreviewPull>();
  // This only identifies an obsolete in-flight read when scheduling a new
  // context. It never gates the new context or confirms native cancellation.
  private readonly inFlight = new Map<string, PreviewPull>();

  constructor(private readonly readLatest: ReadLatest) {}

  private projection(pull: PreviewPull): PreviewProjection {
    const native = pull.nativeProjection, notice = pull.transportNotice;
    if (!notice || clearsFrame(native)) return native;
    return {
      ...native,
      state: notice.state,
      permission: notice.denied ? 'denied' : native.permission,
      errorCode: notice.code,
      detail: '窗口预览读取暂未完成；当前画面不是实时核验结果，可重试。',
    };
  }

  private deliver(pull: PreviewPull, token: symbol, update: PreviewUpdate) {
    const subscription = pull.listeners.get(token);
    if (!pull.active || !subscription) return;
    if (subscription.targetIdentityHash !== null && subscription.targetIdentityHash !== update.projection.targetIdentityHash) return;
    if (update.frame) {
      if (subscription.sequence !== null && BigInt(update.frame.sequence) <= BigInt(subscription.sequence)) return;
      subscription.sequence = update.frame.sequence;
      subscription.targetIdentityHash = update.frame.targetIdentityHash;
    }
    try { void Promise.resolve(subscription.listener(update)).catch(() => {}); } catch { /* Isolate this listener. */ }
  }

  private emit(pull: PreviewPull, frame: PreviewFrame | null) {
    const update = { projection: this.projection(pull), frame };
    for (const token of [...pull.listeners.keys()]) this.deliver(pull, token, update);
  }

  private retire(pull: PreviewPull) {
    pull.active = false;
    clearTimeout(pull.timer); pull.timer = undefined;
    pull.listeners.clear(); pull.lastFrame = null;
    pull.transportNotice = undefined; pull.observed = false;
  }

  private schedule(leaseId: string, pull: PreviewPull, delay: number) {
    pull.timer = setTimeout(() => { pull.timer = undefined; void this.pull(leaseId, pull); }, delay);
  }

  private async pull(leaseId: string, pull: PreviewPull): Promise<void> {
    if (!pull.active || !pull.listeners.size || pull.pending || this.pulls.get(leaseId) !== pull) return;
    try {
      pull.pending = this.readLatest(leaseId, pull.nativeProjection.generation, pull.sequence);
      this.inFlight.set(leaseId, pull);
      const update = await pull.pending;
      if (!pull.active || this.pulls.get(leaseId) !== pull) return;
      const frame = update.frame;
      const validFrame = !frame || frame.leaseId === leaseId && frame.generation === pull.nativeProjection.generation && frame.targetIdentityHash === pull.nativeProjection.targetIdentityHash && /^(0|[1-9]\d*)$/.test(frame.sequence) && (pull.sequence === null || BigInt(frame.sequence) > BigInt(pull.sequence));
      // First unknown-target resolution requires its matching validated frame;
      // metadata alone cannot rebind a known target or manufacture that frame.
      const resolvesTarget = pull.nativeProjection.targetIdentityHash === null && update.projection.state === 'live' && update.projection.permission === 'granted' && /^[0-9a-f]{64}$/.test(update.projection.targetIdentityHash ?? '') && frame?.targetIdentityHash === update.projection.targetIdentityHash && frame.encoding === 'jpeg' && Number.isSafeInteger(frame.width) && frame.width > 0 && Number.isSafeInteger(frame.height) && frame.height > 0 && frame.payload.length > 0 && /^(0|[1-9]\d*)$/.test(frame.sequence) && BigInt(frame.sequence) > 0n && frame.leaseId === leaseId && frame.generation === pull.nativeProjection.generation && (pull.sequence === null || BigInt(frame.sequence) > BigInt(pull.sequence));
      if (sameOwner(update.projection, pull.nativeProjection) && (sameContext(update.projection, pull.nativeProjection) && validFrame || resolvesTarget)) {
        if (frame) pull.sequence = frame.sequence;
        pull.nativeProjection = update.projection;
        if (frame) pull.lastFrame = frame;
        if (clearsFrame(update.projection)) pull.lastFrame = null;
        // A successful metadata read is not evidence that the retained JPEG
        // is live again. Only an admitted live frame clears local degradation.
        if (frame && update.projection.state === 'live' && update.projection.permission === 'granted') pull.transportNotice = undefined;
        pull.observed = true;
        this.emit(pull, frame);
      }
    } catch (cause) {
      if (!pull.active || this.pulls.get(leaseId) !== pull) return;
      const code = cause && typeof cause === 'object' && 'code' in cause && typeof cause.code === 'string' ? cause.code : 'preview_transport';
      const denied = code === 'preview_permission_denied' || code === 'preview_permission_revoked';
      const closed = code === 'preview_target_closed';
      const invalid = code === 'preview_generation_conflict' || code === 'preview_target_identity' || code === 'preview_selection_expired';
      pull.transportNotice = { state: denied ? 'permission_revoked' : closed ? 'target_closed' : invalid ? 'unavailable' : 'stale', denied, code };
      if (denied || closed || invalid) pull.lastFrame = null;
      pull.observed = true;
      this.emit(pull, null);
    } finally {
      if (this.inFlight.get(leaseId) === pull) this.inFlight.delete(leaseId);
      pull.pending = undefined;
    }
    if (pull.active && pull.listeners.size && this.pulls.get(leaseId) === pull) this.schedule(leaseId, pull, 500);
  }

  attach(projection: PreviewProjection, listener: (update: PreviewUpdate) => void): () => void {
    const leaseId = projection.leaseId;
    if (!leaseId) return () => {};
    let pull = this.pulls.get(leaseId);
    const joinsResolvingContext = pull && sameOwner(pull.nativeProjection, projection) && (projection.targetIdentityHash === null || pull.nativeProjection.targetIdentityHash === null);
    if (pull && sameOwner(pull.nativeProjection, projection) && !sameContext(pull.nativeProjection, projection) && !joinsResolvingContext) throw new Error('预览目标身份与当前订阅不一致，请重新读取关联。');
    if (!pull || !sameContext(pull.nativeProjection, projection) && !joinsResolvingContext) {
      if (pull) {
        const conflict = { projection: { ...pull.nativeProjection, state: 'unavailable' as const, errorCode: 'preview_generation_conflict', detail: '预览上下文已更新，请读取当前关联。' }, frame: null };
        for (const token of [...pull.listeners.keys()]) this.deliver(pull, token, conflict);
        this.retire(pull);
      }
      pull = { nativeProjection: projection, lastFrame: null, observed: false, active: true, sequence: null, listeners: new Map() };
      this.pulls.set(leaseId, pull);
    }
    if (pull.listeners.size >= 4) throw new Error('同一窗口最多支持四个同时预览的订阅。');
    const token = Symbol('preview-subscription'), current = pull;
    current.listeners.set(token, { listener, sequence: null, targetIdentityHash: projection.targetIdentityHash });
    if (current.observed) {
      const cached = { projection: this.projection(current), frame: current.lastFrame }, sequence = current.sequence, native = current.nativeProjection, notice = current.transportNotice;
      queueMicrotask(() => {
        if (current.active && this.pulls.get(leaseId) === current && current.sequence === sequence && current.nativeProjection === native && current.transportNotice === notice) this.deliver(current, token, cached);
      });
    }
    if (!current.timer) {
      // One independent next-task kickoff preserves immediate detach/release
      // ordering, while an obsolete invocation may remain unsettled forever.
      if (this.inFlight.has(leaseId) && this.inFlight.get(leaseId) !== current) this.schedule(leaseId, current, 0);
      else void this.pull(leaseId, current);
    }
    let subscribed = true;
    return () => {
      if (!subscribed) return; subscribed = false; current.listeners.delete(token);
      if (current.listeners.size || this.pulls.get(leaseId) !== current) return;
      this.retire(current); this.pulls.delete(leaseId);
    };
  }
}
