import type { ApplicationStore } from '../../bridge/projections';
import { useApplication } from '../../bridge/projections';
import { WindowFrame, WindowLoading } from '../../ui/patterns/WindowFrame';
import { StudyScreen } from '../../features/study';
import { Notice } from '../../ui/patterns/Patterns';

export function LearningWindow({ store }: { store: ApplicationStore }) {
  const { snapshot, error } = useApplication(store);
  return <WindowFrame title="查词与学习">{snapshot ? <>{error ? <Notice error>{error}</Notice> : null}<StudyScreen snapshot={snapshot} store={store}/></> : <WindowLoading error={error} onRetry={() => void store.refresh()}/>}</WindowFrame>;
}
