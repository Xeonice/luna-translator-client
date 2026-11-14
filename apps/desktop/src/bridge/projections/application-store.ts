import { useEffect, useSyncExternalStore } from 'react';
import type { AppSnapshot, Command, ProjectionUpdate } from '../contracts.generated';
import { commandEnvelope, errorMessage } from '../application-port';
import type { ApplicationPort } from '../application-port';

type StoreState = { snapshot: AppSnapshot | null; loading: boolean; error: string | null };
/** Window-local projection only. Business ownership and subscription revision remain in Rust. */
export class ApplicationStore {
  private state: StoreState = { snapshot: null, loading: true, error: null };
  private listeners = new Set<() => void>();
  private lifetime = 0;
  private stop: (() => void) | undefined;
  private references = 0;
  private refreshToken = 0;
  constructor(readonly port: ApplicationPort) {}
  getSnapshot = () => this.state;
  subscribe = (listener: () => void) => { this.listeners.add(listener); return () => this.listeners.delete(listener); };
  private publish(state: StoreState) { this.state = state; this.listeners.forEach(listener => listener()); }
  async refresh() {
    const token = ++this.refreshToken;
    try {
      const snapshot = await this.port.snapshot();
      if (token !== this.refreshToken) return;
      const current = this.state.snapshot;
      if (current && snapshot.serverInstanceId === current.serverInstanceId && snapshot.profileGeneration === current.profileGeneration && snapshot.revision < current.revision) return;
      this.publish({ snapshot, loading: false, error: null });
    } catch (error) { if (token === this.refreshToken) this.publish({ ...this.state, loading: false, error: errorMessage(error) }); }
  }
  private update = (update: ProjectionUpdate) => {
    const current = this.state.snapshot;
    if (!current || current.serverInstanceId !== update.serverInstanceId || current.profileGeneration !== update.profileGeneration || update.baseRevision !== current.revision) { void this.refresh(); return; }
    if (update.revision <= current.revision) return;
    if (update.snapshot.revision !== update.revision || update.snapshot.serverInstanceId !== update.serverInstanceId || update.snapshot.profileGeneration !== update.profileGeneration) { this.publish({ ...this.state, error: '收到的视图版本无效，正在重新同步。' }); void this.refresh(); return; }
    this.publish({ snapshot: update.snapshot, loading: false, error: null });
  };
  connect() {
    this.references++;
    if (this.references === 1) {
      const lifetime = ++this.lifetime;
      void this.port.subscribe(this.update, error => { if (this.lifetime === lifetime) this.publish({ ...this.state, loading: false, error: errorMessage(error) }); }).then(stop => { if (this.lifetime === lifetime && this.references > 0) this.stop = stop; else stop(); }).catch(() => {});
      void this.refresh();
    }
    return () => { this.references--; if (this.references === 0) { this.lifetime++; this.refreshToken++; this.stop?.(); this.stop = undefined; } };
  }
  async command(command: Command) { const receipt = await this.port.dispatch(commandEnvelope(command, this.state.snapshot)); await this.refresh(); return receipt; }
}
export function useApplication(store: ApplicationStore) {
  useEffect(() => store.connect(), [store]);
  return useSyncExternalStore(store.subscribe, store.getSnapshot);
}
