import { useCallback, useEffect, useRef, useState } from 'react';
import { errorMessage } from '../../bridge/application-port';

type Page<T> = { items: T[]; nextCursor: string | null };
type Intent = { owner: string; scope: string; cursor: string | null; mode: 'replace' | 'append' };
type QueryState<T> = {
  owner: string;
  scope: string | null;
  items: T[];
  cursor: string | null;
  initialized: boolean;
  pending: Intent | null;
  failure: (Intent & { message: string }) | null;
};
const empty = <T,>(owner: string): QueryState<T> => ({ owner, scope: null, items: [], cursor: null, initialized: false, pending: null, failure: null });

/** UI query bookkeeping only. The opaque cursor and filtering remain owned by Rust. */
export function useHistoryQuery<T>({ owner, scope, active, blocked, fetchPage, keyOf }: {
  owner: string;
  scope: string;
  active: boolean;
  blocked: boolean;
  fetchPage: (scope: string, cursor: string | null) => Promise<Page<T>>;
  keyOf: (item: T) => string;
}) {
  const [state, setState] = useState<QueryState<T>>(() => empty(owner));
  const current = useRef({ owner, scope, active, blocked });
  current.current = { owner, scope, active, blocked };
  const stateRef = useRef(state);
  stateRef.current = state;
  const fetchRef = useRef(fetchPage);
  fetchRef.current = fetchPage;
  const keyRef = useRef(keyOf);
  keyRef.current = keyOf;
  const epoch = useRef(0);
  const inFlight = useRef<Intent | null>(null);

  const run = useCallback(async (intent: Intent) => {
    const context = current.current;
    if (!context.active || context.blocked || context.owner !== intent.owner || context.scope !== intent.scope) return;
    const ticket = ++epoch.current;
    inFlight.current = intent;
    setState(previous => ({ ...(previous.owner === intent.owner ? previous : empty<T>(intent.owner)), pending: intent, failure: null }));
    const isCurrent = () => epoch.current === ticket && current.current.active && !current.current.blocked && current.current.owner === intent.owner && current.current.scope === intent.scope;
    try {
      const result = await fetchRef.current(intent.scope, intent.cursor);
      if (!isCurrent()) return;
      setState(previous => {
        if (intent.mode === 'append' && (previous.owner !== intent.owner || previous.scope !== intent.scope || previous.cursor !== intent.cursor)) return previous;
        const items = intent.mode === 'append' ? [...previous.items] : [];
        const seen = new Set(items.map(keyRef.current));
        for (const item of result.items) {
          const key = keyRef.current(item);
          if (!seen.has(key)) { items.push(item); seen.add(key); }
        }
        return { owner: intent.owner, scope: intent.scope, items, cursor: result.nextCursor, initialized: true, pending: null, failure: null };
      });
    } catch (error) {
      if (isCurrent()) setState(previous => ({ ...(previous.owner === intent.owner ? previous : empty<T>(intent.owner)), pending: null, failure: { ...intent, message: errorMessage(error) } }));
    } finally {
      if (isCurrent()) {
        inFlight.current = null;
        setState(previous => previous.pending === intent ? { ...previous, pending: null } : previous);
      }
    }
  }, []);

  useEffect(() => {
    if (!active || blocked) return;
    const previous = stateRef.current;
    if (previous.owner !== owner || previous.scope !== scope || !previous.initialized) {
      void run({ owner, scope, cursor: null, mode: 'replace' });
    } else if (previous.pending) {
      // Resume an interrupted append with its original cursor, not a fresh first page.
      void run(previous.pending);
    }
    return () => { epoch.current += 1; inFlight.current = null; };
  }, [active, blocked, owner, scope, run]);

  const refresh = useCallback(() => {
    const context = current.current;
    if (context.blocked || !context.active) return;
    void run({ owner: context.owner, scope: context.scope, cursor: null, mode: 'replace' });
  }, [run]);
  const more = useCallback(() => {
    const context = current.current, previous = stateRef.current;
    if (context.blocked || !context.active || inFlight.current || previous.owner !== context.owner || previous.scope !== context.scope || !previous.cursor) return;
    void run({ owner: context.owner, scope: context.scope, cursor: previous.cursor, mode: 'append' });
  }, [run]);
  const retry = useCallback(() => {
    const context = current.current, failure = stateRef.current.failure;
    if (context.blocked || !context.active || inFlight.current || !failure || failure.owner !== context.owner || failure.scope !== context.scope) return;
    void run(failure);
  }, [run]);
  const visible = state.owner === owner ? state : empty<T>(owner);
  return {
    ...visible,
    loading: visible.pending !== null && visible.pending.scope === scope,
    previousScope: visible.initialized && visible.scope !== scope,
    canAppend: visible.initialized && visible.scope === scope && visible.cursor !== null,
    refresh, more, retry,
  };
}
