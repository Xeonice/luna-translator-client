/** Test-only component host. It composes production components, never draws a
 * design/expected image or substitutes a renderer for a product component. */
import { useEffect, useRef, useState } from 'react';
import type { PreviewProjection, SessionDataset } from '../../src/bridge/contracts.generated';
import { useApplication } from '../../src/bridge/projections';
import type { ApplicationStore } from '../../src/bridge/projections';
import { Select } from '../../src/ui/primitives/Controls';
import { WindowPreview } from '../../src/features/capture';
import { SessionRecordPanel } from '../../src/features/reading/SessionRecordPanel';
import type { DesignPort } from './design-port';

/** The controlled candidate owner supplies an explicit lease before mounting.
 * WindowPreview still owns only its subscription and production rendering. */
type PreviewPresentation = { title?: string; detail?: string; identityDetail?: string; width?: number; targetFacts?: import('../../src/features/capture/PreviewDetails').PreviewTargetFacts };
function reviewedPresentation(): PreviewPresentation {
  return Reflect.get(window, '__reviewedPreviewInputs') ?? {};
}
type PreparedPreview = { candidateId: string; projection: PreviewProjection };
function ReviewedPreview({ store, fixture: suppliedFixture, consumer = 'candidate' }: { store: ApplicationStore; fixture?: PreparedPreview; consumer?: 'candidate' | 'session' }) {
  const port = store.port as DesignPort;
  const fixture = suppliedFixture ?? port.reviewedCandidatePreviewFixture;
  if (!fixture) throw new Error('Reviewed preview requires an explicit pre-render owner fixture');
  const [projection, setProjection] = useState<PreviewProjection>(fixture.projection);
  const [presentation, setPresentation] = useState<PreviewPresentation>(reviewedPresentation);
  useEffect(() => {
    const update = (value: PreviewPresentation) => setPresentation(current => ({ ...current, ...value }));
    Object.assign(window, { __setReviewedPreviewInputs: update });
    return () => { if (Reflect.get(window, '__setReviewedPreviewInputs') === update) Reflect.deleteProperty(window, '__setReviewedPreviewInputs'); };
  }, []);
  const latest = useRef(projection);
  const receive = (value: PreviewProjection) => { latest.current = value; setProjection(value); };
  const intent = async (kind: 'retry' | 'select' | 'pause' | 'resume') => {
    const previous = latest.current;
    if (kind === 'select') return port.selectWindowPreview({ candidateId: fixture.candidateId, consumer: 'candidate', consumerId: previous.consumerId, expectedGeneration: previous.generation });
    if (!previous.leaseId) throw new Error('Reviewed candidate owner has no prepared lease');
    return port.controlWindowPreview({ leaseId: previous.leaseId, generation: previous.generation, action: kind === 'pause' ? 'pause' : 'resume' });
  };
  return <WindowPreview port={port} candidateId={consumer === 'session' ? null : fixture.candidateId} consumer={consumer} consumerId={projection.consumerId} candidateProjection={projection} onProjection={receive} onPreviewIntent={consumer === 'candidate' ? intent : undefined} title={presentation.title ?? 'サクラノ詩 10th Anniversary edition'} detail={presentation.detail ?? 'Sakura-no-Uta-10th · sakuranouta10th.exe'} identityDetail={presentation.identityDetail ?? 'design-window-101 · PID 6200'} targetFacts={presentation.targetFacts}/>;
}

export function ReviewedSurfaceProbe({ store, surface, longList }: { store: ApplicationStore; surface: string; longList: boolean }) {
  const { snapshot } = useApplication(store);
  const [value, setValue] = useState('body');
  useEffect(() => { document.documentElement.dataset.theme = snapshot?.preferences.theme ?? 'dark'; document.documentElement.dataset.transparency = snapshot?.preferences.transparencyMode ?? 'material'; }, [snapshot?.preferences.theme, snapshot?.preferences.transparencyMode]);
  if (!snapshot) return null;
  if (surface === 'select') return <div style={{ position:'fixed',left:24,top:24,width:340,display:'grid',fontSize:12 }}><Select label="组件示例文本来源" hideLabel value={value} onChange={setValue} options={[
    {value:'body',label:'Artemis · 正文'},{value:'unselected',label:'Artemis · 界面文本'},{value:'long',label:'GameN7_Steam.exe · Steam · MAGES2 · 长文本来源名称完整显示'},{value:'disabled',label:'已退出来源 · 不可选择',disabled:true},
    ...(longList?Array.from({length:40},(_,index)=>({value:`controlled-${index}`,label:`完整会话来源 ${index+1}`})):[]),
  ]}/></div>;
  if (surface === 'preview-pair') {
    const pair = Reflect.get(store.port, 'reviewedPreviewPairFixtures') as { candidate: PreparedPreview; session: PreparedPreview } | undefined;
    if (!pair) throw new Error('Two-preview host requires two explicit owners');
    return <div style={{ position:'fixed',left:24,top:8,display:'flex',gap:24 }}><div data-testid="preview-pair-candidate" style={{width:282}}><ReviewedPreview store={store} fixture={pair.candidate}/></div><div data-testid="preview-pair-session" style={{width:282}}><ReviewedPreview store={store} fixture={pair.session} consumer="session"/></div></div>;
  }
  if (surface === 'preview') return <div style={{ position:'fixed',left:24,top:8,width:reviewedPresentation().width ?? 282 }}><ReviewedPreview store={store}/></div>;
  const dataset: SessionDataset = surface === 'session-hook-events' ? 'hook_events' : 'extraction';
  return <div data-reviewed-session-surface style={{ position:'fixed',left:24,top:24,width:848 }}><SessionRecordPanel snapshot={snapshot} store={store} sessionId={snapshot.session?.sessionId ?? 'controlled-approved-session'} dataset={dataset}/></div>;
}
