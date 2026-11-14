import { useCallback, useEffect, useRef, useState } from 'react';
import type { AppSnapshot, ReadingUnit, ReadingUnitProjection, TaskView } from '../../bridge/contracts.generated';
import type { ApplicationStore } from '../../bridge/projections';
import { errorMessage } from '../../bridge/application-port';

const REREAD_INTERVAL_MS = 100;

export interface ReadingProjectionView {
  value: ReadingUnitProjection | null;
  loading: boolean;
  refreshing: boolean;
  error: string | null;
  retry: () => void;
}

interface Context {
  key: string;
  active: boolean;
  desiredCommit: number;
  appliedCommit: number;
  blockedCommit: number;
  lastStarted: number;
  pending: object | null;
  timer: ReturnType<typeof setTimeout> | null;
  value: ReadingUnitProjection | null;
  error: string | null;
  force: boolean;
  request: (force?: boolean) => void;
}

interface StoredView {
  key: string;
  value: ReadingUnitProjection | null;
  loading: boolean;
  refreshing: boolean;
  error: string | null;
}

/** This cache owns only a view subscription. Rust selects engines, heads and
 * statuses for the fixed object, including objects outside snapshot history. */
export function useReadingUnitProjection(snapshot: AppSnapshot, store: ApplicationStore, unit: ReadingUnit, active = true): ReadingProjectionView {
  const key = JSON.stringify([snapshot.profileId, snapshot.serverInstanceId, snapshot.profileGeneration, unit.unitId, unit.textRevision, unit.sourceHash]);
  const latest = useRef({ key, active, store, snapshot });
  latest.current = { key, active, store, snapshot };
  const context = useRef<Context | null>(null);
  const [view, setView] = useState<StoredView>({ key, value: null, loading: active, refreshing: false, error: null });

  useEffect(() => {
    if (!active) {
      context.current = null;
      setView({ key, value: null, loading: false, refreshing: false, error: null });
      return;
    }
    const identity = { profileId: snapshot.profileId, serverInstanceId: snapshot.serverInstanceId, profileGeneration: snapshot.profileGeneration, unitId: unit.unitId, textRevision: unit.textRevision, sourceHash: unit.sourceHash, sessionId: unit.sessionId, sourceText: unit.sourceText };
    const ctx: Context = { key, active: true, desiredCommit: snapshot.commitRevision, appliedCommit: -1, blockedCommit: -1, lastStarted: -Infinity, pending: null, timer: null, value: null, error: null, force: false, request: () => {} };
    context.current = ctx;
    const owns = () => ctx.active && context.current === ctx && latest.current.key === key && latest.current.active && latest.current.store === store;
    const emit = () => {
      if (!owns()) return;
      setView({ key, value: ctx.value, loading: ctx.value === null && ctx.error === null, refreshing: ctx.value !== null && (ctx.pending !== null || ctx.timer !== null), error: ctx.error });
    };
    const validate = (value: ReadingUnitProjection, floor: number) => {
      if (value.profileId !== identity.profileId || value.serverInstanceId !== identity.serverInstanceId || value.profileGeneration !== identity.profileGeneration || value.unit.unitId !== identity.unitId || value.unit.textRevision !== identity.textRevision || value.unit.sourceHash !== identity.sourceHash || value.unit.sessionId !== identity.sessionId || value.unit.sourceText !== identity.sourceText || value.translationState.unitId !== identity.unitId || value.translationState.textRevision !== identity.textRevision || value.translationState.sourceHash !== identity.sourceHash) throw new Error('翻译状态回执不属于当前原句，请重新读取。');
      if (!Number.isSafeInteger(value.commitRevision) || value.commitRevision < floor || value.commitRevision < ctx.appliedCommit) throw new Error('翻译状态回执已过时，请重新读取。');
      const taskMatches = (task: TaskView) => task.unitId === identity.unitId && task.textRevision === identity.textRevision && task.purpose === 'translation';
      for (const engine of value.translationState.engines) {
        if (engine.status.kind === 'current' && !taskMatches(engine.status.task) || engine.status.kind === 'unordered' && engine.status.activeTask && !taskMatches(engine.status.activeTask)) throw new Error('翻译请求回执不属于当前原句，请重新读取。');
      }
    };
    const schedule = () => {
      if (!owns() || ctx.pending || ctx.timer) return;
      if (!ctx.force && (ctx.desiredCommit <= ctx.appliedCommit || ctx.desiredCommit <= ctx.blockedCommit)) { emit(); return; }
      const delay = Math.max(0, Math.ceil(REREAD_INTERVAL_MS - (performance.now() - ctx.lastStarted)));
      if (delay > 0) {
        ctx.timer = setTimeout(() => { ctx.timer = null; begin(); }, delay);
        emit();
      } else begin();
    };
    const begin = () => {
      if (!owns() || ctx.pending) return;
      // A browser timer may wake early. Recheck the start interval rather than
      // allowing timer rounding to turn dirty commits into a rapid read loop.
      if (performance.now() - ctx.lastStarted < REREAD_INTERVAL_MS) { schedule(); return; }
      const request = {};
      ctx.desiredCommit = Math.max(ctx.desiredCommit, latest.current.snapshot.commitRevision);
      const floor = ctx.desiredCommit;
      ctx.pending = request;
      ctx.lastStarted = performance.now();
      ctx.force = false;
      ctx.error = null;
      emit();
      void store.port.readingUnitProjection(identity.unitId, identity.textRevision).then(value => {
        if (!owns() || ctx.pending !== request) return;
        validate(value, floor);
        // A request-start floor is deliberate: continuously arriving unrelated
        // commits must not starve a valid as-of response from becoming visible.
        ctx.value = value;
        ctx.appliedCommit = value.commitRevision;
        ctx.blockedCommit = -1;
      }).catch(cause => {
        if (!owns() || ctx.pending !== request) return;
        ctx.error = errorMessage(cause);
        // A stale/failed read never drives a zero-delay self-retry loop.
        ctx.blockedCommit = ctx.desiredCommit;
      }).finally(() => {
        if (!owns() || ctx.pending !== request) return;
        ctx.pending = null;
        schedule();
        emit();
      });
    };
    ctx.request = (force = false) => {
      if (!owns()) return;
      ctx.desiredCommit = Math.max(ctx.desiredCommit, latest.current.snapshot.commitRevision);
      if (force) { ctx.force = true; ctx.blockedCommit = -1; }
      schedule();
    };
    ctx.request();
    return () => {
      ctx.active = false;
      if (ctx.timer !== null) clearTimeout(ctx.timer);
      ctx.timer = null;
      if (context.current === ctx) context.current = null;
    };
    // Identity/visibility changes create a new owner. Commit changes use the
    // separate coalescing effect below, so an unsettled read is never replaced.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, active, store]);

  useEffect(() => { if (active && context.current?.key === key) context.current.request(); }, [active, key, store, snapshot.commitRevision]);
  const retry = useCallback(() => { if (context.current?.key === key && latest.current.active) context.current.request(true); }, [key]);
  if (!active || view.key !== key) return { value: null, loading: active, refreshing: false, error: null, retry };
  return { value: view.value, loading: view.loading, refreshing: view.refreshing || view.value !== null && snapshot.commitRevision > view.value.commitRevision && view.error === null, error: view.error, retry };
}

/** The query chose the task/attempt. This joins only its Rust stream buffer;
 * bounded snapshot tasks never decide current state, engine or error. */
export function readingPartialText(snapshot: AppSnapshot, projection: ReadingUnitProjection, task: TaskView): string | null {
  if (!task.attemptId || !['queued', 'running'].includes(task.status) || snapshot.profileId !== projection.profileId || snapshot.serverInstanceId !== projection.serverInstanceId || snapshot.profileGeneration !== projection.profileGeneration || snapshot.commitRevision < projection.commitRevision) return null;
  return snapshot.tasks.find(item => item.taskId === task.taskId && item.attemptId === task.attemptId && item.unitId === projection.unit.unitId && item.textRevision === projection.unit.textRevision)?.partialText ?? null;
}
