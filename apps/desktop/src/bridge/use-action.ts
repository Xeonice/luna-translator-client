import { useEffect, useRef, useState } from 'react';
import { errorMessage } from './application-port';

export function useAction() {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const alive = useRef(true);
  const running = useRef(false);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  const run = async (action: () => Promise<unknown>) => {
    if (running.current) return false;
    running.current = true;
    setPending(true); setError(null);
    try { await action(); return true; } catch (value) { if (alive.current) setError(errorMessage(value)); return false; } finally { running.current = false; if (alive.current) setPending(false); }
  };
  return { pending, error, run, clearError: () => setError(null) };
}
