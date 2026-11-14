import { useCallback, useEffect, useRef, useState } from 'react';
import type { ApplicationPort } from '../../bridge/application-port';
import type { GoogleAuthorizationProjection, GoogleAuthorizationRef, GoogleInteractiveContext, GoogleLoginOperationProjection, GoogleLoginOperationQuery, GoogleRevokeProjection } from '../../bridge/contracts.generated';

type RunningLogin = { key: string; query: GoogleLoginOperationQuery; cancelRequested: boolean; timer?: ReturnType<typeof setTimeout> };
type View = { key: string | null; projection: GoogleAuthorizationProjection | null; operation: GoogleLoginOperationProjection | null; loading: boolean; revoking: boolean; error: string | null };
const idle: View = { key: null, projection: null, operation: null, loading: false, revoking: false, error: null };

/** Own only transport/view lifetime. Rust alone validates clients, grants and terminal cleanup.
 * Opening/editing/saving never starts OAuth; only start() represents an explicit click. */
export function useGoogleAuthorization({ port, editorKey, context, active, onAuthorized, onRevoked }: {
  port: ApplicationPort;
  editorKey: string | null;
  context: GoogleInteractiveContext | null;
  active: boolean;
  onAuthorized: (authorization: GoogleAuthorizationRef) => void;
  onRevoked: (result: GoogleRevokeProjection) => void;
}) {
  const key = active && context && editorKey ? JSON.stringify([editorKey, context]) : null;
  const currentKey = useRef(key); currentKey.current = key;
  const currentContext = useRef(context); currentContext.current = context;
  const callbacks = useRef({ onAuthorized, onRevoked }); callbacks.current = { onAuthorized, onRevoked };
  const running = useRef<RunningLogin | null>(null), mounted = useRef(false), inspectEpoch = useRef(0), mutationEpoch = useRef(0);
  const [view, setView] = useState<View>(idle), [refreshKey, setRefreshKey] = useState(0);
  const isCurrent = useCallback((operation: RunningLogin) => mounted.current && currentKey.current === operation.key && running.current === operation, []);
  const abandon = useCallback((operation: RunningLogin) => {
    if (operation.timer) clearTimeout(operation.timer);
    if (running.current === operation) running.current = null;
    // Native owns the task until physical drain. A lost/late reply is never a local success.
    void port.cancelGoogleLogin(operation.query).catch(() => {});
  }, [port]);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; inspectEpoch.current++; mutationEpoch.current++; const operation = running.current; if (operation) abandon(operation); }; }, [abandon]);
  useEffect(() => {
    const generation = ++inspectEpoch.current;
    mutationEpoch.current++;
    const operation = running.current;
    if (operation && operation.key !== key) abandon(operation);
    if (!key || !context) { setView(idle); return; }
    setView({ ...idle, key, loading: true });
    void port.inspectGoogleAuthorization(structuredClone(context)).then(projection => {
      if (mounted.current && currentKey.current === key && inspectEpoch.current === generation) setView({ ...idle, key, projection });
    }).catch(() => {
      if (mounted.current && currentKey.current === key && inspectEpoch.current === generation) setView({ ...idle, key, error: '无法核对 Google 授权状态；当前输入已保留，请重新读取。' });
    });
    return () => { if (inspectEpoch.current === generation) inspectEpoch.current++; };
  }, [key, port, abandon, refreshKey]);

  const accept = useCallback((operation: RunningLogin, value: GoogleLoginOperationProjection) => {
    if (!isCurrent(operation)) return false;
    if (value.operationId !== operation.query.operationId) throw new Error('Google operation identity mismatch');
    if (value.state === 'authorized' && !value.authorization) throw new Error('Google authorization reference missing');
    if (value.state !== 'authorizing') {
      if (operation.timer) clearTimeout(operation.timer);
      running.current = null; // Stop before the editor receives its new reference.
    }
    setView(old => old.key === operation.key ? { ...old, operation: value, error: null } : old);
    if (value.state === 'authorized' && value.authorization) callbacks.current.onAuthorized(value.authorization);
    return value.state === 'authorizing';
  }, [isCurrent]);
  const poll = useCallback((operation: RunningLogin) => {
    const next = async () => {
      if (!isCurrent(operation)) return;
      try {
        const value = await port.googleLoginStatus(operation.query);
        if (!accept(operation, value)) return;
      } catch {
        if (!isCurrent(operation)) return;
        setView(old => old.key === operation.key ? { ...old, error: '暂时无法读取授权结果；正在核对安全收尾状态，尚未采用授权。' } : old);
        if (!operation.cancelRequested) {
          operation.cancelRequested = true;
          void port.cancelGoogleLogin(operation.query).catch(() => {});
        }
      }
      if (isCurrent(operation)) operation.timer = setTimeout(() => void next(), 500);
    };
    operation.timer = setTimeout(() => void next(), 500);
  }, [accept, isCurrent, port]);
  const start = useCallback(async () => {
    const before = currentContext.current;
    const identity = currentKey.current;
    if (!before || !identity || running.current || view.key !== identity || !view.projection?.canLogin || view.loading || view.revoking || view.operation?.state === 'cleanup_pending') return;
    const operation: RunningLogin = { key: identity, query: { operationId: crypto.randomUUID(), expectedProfileGeneration: before.expectedProfileGeneration }, cancelRequested: false };
    running.current = operation;
    setView(old => ({ ...old, error: null, operation: { operationId: operation.query.operationId, state: 'authorizing', message: '正在请求系统浏览器授权…', canCancel: true } }));
    try {
      const value = await port.startGoogleLogin({ operationId: operation.query.operationId, context: structuredClone(before) });
      if (!isCurrent(operation)) { void port.cancelGoogleLogin(operation.query).catch(() => {}); return; }
      if (accept(operation, value)) poll(operation);
    } catch {
      if (!isCurrent(operation)) return;
      // The request may have reached Native even if its reply was lost. Ask it
      // to cancel and keep observing the actual result instead of inventing one.
      operation.cancelRequested = true;
      setView(old => old.key === operation.key ? { ...old, error: '未能确认浏览器授权已启动；正在请求安全收尾。' } : old);
      try { const value = await port.cancelGoogleLogin(operation.query); if (accept(operation, value)) poll(operation); }
      catch { if (isCurrent(operation)) { running.current = null; setView(old => old.key === operation.key ? { ...old, operation: null, error: '无法确认本次授权操作，请重新读取状态后再试。' } : old); } }
    }
  }, [accept, isCurrent, poll, port, view]);
  const cancel = useCallback(async () => {
    const operation = running.current;
    if (!operation || operation.cancelRequested || !isCurrent(operation)) return;
    operation.cancelRequested = true;
    setView(old => old.key === operation.key && old.operation ? { ...old, operation: { ...old.operation, canCancel: false, message: '正在取消并等待安全收尾…' } } : old);
    try { accept(operation, await port.cancelGoogleLogin(operation.query)); }
    catch {
      if (isCurrent(operation)) { operation.cancelRequested = false; setView(old => old.key === operation.key ? { ...old, error: '取消请求尚未确认；请重试，仍以实际终态为准。', operation: old.operation ? { ...old.operation, canCancel: true } : null } : old); }
    }
  }, [accept, isCurrent, port]);
  const revoke = useCallback(async () => {
    const before = currentContext.current, identity = currentKey.current;
    if (!before || !identity || running.current || view.key !== identity || !view.projection?.canLogout || !view.projection.authorization || view.revoking) return;
    const generation = ++mutationEpoch.current;
    setView(old => ({ ...old, revoking: true, error: null }));
    try {
      const result = await port.revokeGoogleAuthorization({ operationId: crypto.randomUUID(), expectedProfileGeneration: before.expectedProfileGeneration, authorization: view.projection.authorization, ...(before.proxy ? { proxy: before.proxy } : {}) });
      if (!mounted.current || currentKey.current !== identity || mutationEpoch.current !== generation) return;
      if (result.localRevoked) callbacks.current.onRevoked(result);
      else setView(old => old.key === identity ? { ...old, error: result.message } : old);
    } catch { if (mounted.current && currentKey.current === identity && mutationEpoch.current === generation) setView(old => old.key === identity ? { ...old, error: '解除授权未完整完成；请重新核对实际状态。' } : old); }
    finally { if (mounted.current && currentKey.current === identity && mutationEpoch.current === generation) setView(old => old.key === identity ? { ...old, revoking: false } : old); }
  }, [port, view]);
  const current = view.key === key ? view : { ...idle, key, loading: key !== null };
  return { ...current, pending: current.operation?.state === 'authorizing', reload: () => setRefreshKey(value => value + 1), start, cancel, revoke };
}
