/** Finite validation-only context. Normal production bootstrap never imports this module. */
export const GENERAL_VISUAL_STATE_IDS = [
  'all-page-settings-general-normal', 'all-page-settings-general-empty',
  'all-page-settings-general-loading', 'all-page-settings-general-error',
  'all-page-settings-general-disabled', 'all-page-settings-general-long-text',
  'all-page-settings-general-light', 'all-page-settings-general-narrow',
  'all-page-settings-general-zoom-200', 'all-page-settings-general-keyboard-focus-escape',
  'all-page-settings-general-scope-global', 'all-page-settings-general-scope-game',
  'all-page-settings-general-restore-inheritance', 'all-page-settings-general-save-failed',
] as const;
export type GeneralVisualStateId = typeof GENERAL_VISUAL_STATE_IDS[number];
export type GeneralVisualContext = Readonly<{
  schemaVersion: 1;
  kind: 'general-native-visual-context/v1';
  transport: 'packaged_native' | 'real_react';
  side: 'reference' | 'candidate';
  stateId: GeneralVisualStateId;
  subcase: 'primary' | 'initial' | 'menu-open' | 'content-end';
  canvasId: string;
  nonce: string;
  width: 1000 | 760;
  height: 720 | 560;
  theme: 'dark' | 'light';
  zoom: 1 | 2;
}>;
const contextKeys = ['schemaVersion','kind','transport','side','stateId','subcase','canvasId','nonce','width','height','theme','zoom'].sort();
const factsKeys = new Set(['title','headings','text','switches','buttons','scope','status','dirty','menuOpen','focused','geometry','scrolls','theme','zoom','fontStatus','documentVisibility','hasFocus','userAgent','replay','errors','renderSource','provenance','actions','controlledCommands','readQueries','saveCalls','startupWrites','forbiddenEffects','caseAssertions','loadedResources','nativeAcl']);
declare global {
  interface Window {
    __LUNA_GENERAL_VISUAL__?: unknown;
    __GENERAL_VISUAL_RESULT__?: Readonly<{context: GeneralVisualContext; facts: Record<string, unknown>; error: string | null}>;
  }
}
export function getGeneralVisualContext(raw: unknown = window.__LUNA_GENERAL_VISUAL__): GeneralVisualContext {
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) throw new Error('Native visual context required');
  const value = raw as Record<string, unknown>;
  if (JSON.stringify(Object.keys(value).sort()) !== JSON.stringify(contextKeys) || value.schemaVersion !== 1 || value.kind !== 'general-native-visual-context/v1'
      || (value.transport !== 'packaged_native' && value.transport !== 'real_react') || (value.side !== 'reference' && value.side !== 'candidate')
      || !GENERAL_VISUAL_STATE_IDS.includes(value.stateId as GeneralVisualStateId) || typeof value.nonce !== 'string' || !/^[0-9a-f]{32}$/.test(value.nonce)) throw new Error('Invalid finite native visual context');
  if (!((value.width === 1000 && value.height === 720) || (value.width === 760 && value.height === 560))
      || (value.theme !== 'dark' && value.theme !== 'light') || (value.zoom !== 1 && value.zoom !== 2)
      || value.canvasId !== `${value.width}x${value.height}-${value.theme}-z${value.zoom}`) throw new Error('Invalid visual canvas');
  if (value.subcase !== 'primary' && !(value.subcase === 'initial' && value.stateId === 'all-page-settings-general-loading')
      && !(value.subcase === 'menu-open' && value.stateId === 'all-page-settings-general-keyboard-focus-escape')
      && !(value.subcase === 'content-end' && value.stateId === 'all-page-settings-general-normal' && value.zoom === 2)) throw new Error('Invalid visual subcase');
  return Object.freeze({...value}) as GeneralVisualContext;
}
function bounded(value: unknown, depth = 0): void {
  if (depth > 8) throw new Error('Visual observation exceeds maximum depth');
  if (value === null || typeof value === 'boolean') return;
  if (typeof value === 'number') { if (!Number.isFinite(value)) throw new Error('Non-finite visual number'); return; }
  if (typeof value === 'string') { if (value.length > 8192) throw new Error('Visual string exceeds limit'); return; }
  if (Array.isArray(value)) { if (value.length > 128) throw new Error('Visual array exceeds limit'); value.forEach(item => bounded(item, depth + 1)); return; }
  if (typeof value === 'object') {
    const entries=Object.entries(value as Record<string,unknown>);
    if (entries.length > 128 || entries.some(([key]) => key.length > 128 || ['__proto__','constructor','prototype'].includes(key))) throw new Error('Invalid visual object');
    entries.forEach(([,item]) => bounded(item, depth + 1)); return;
  }
  throw new Error('Non-JSON visual observation');
}
type NativeBridge={invoke:(command:string,args:unknown)=>Promise<unknown>};
type NativeAclProbe={coreCommand:string;coreDenied:boolean;coreError:string;businessCommand:string;businessUnknown:boolean;businessError:string};
let nativeAclProbe: Promise<NativeAclProbe> | undefined;
function errorText(error: unknown): string { return typeof error==='string'?error:JSON.stringify(error) ?? String(error); }
async function probeNativeAcl(native: NativeBridge): Promise<NativeAclProbe> {
  const result:NativeAclProbe={coreCommand:'plugin:window|scale_factor',coreDenied:false,coreError:'',businessCommand:'application_snapshot',businessUnknown:false,businessError:''};
  try { await native.invoke(result.coreCommand,{label:'general-visual-verification'}); result.coreError='unexpectedly allowed'; }
  catch(error) { result.coreError=errorText(error); result.coreDenied=/not allowed|not permitted|denied|forbidden/i.test(result.coreError)&&!/not found|unknown command/i.test(result.coreError); }
  try { await native.invoke(result.businessCommand,{}); result.businessError='unexpectedly registered'; }
  catch(error) { result.businessError=errorText(error); result.businessUnknown=result.businessError===`Command ${result.businessCommand} not found`; }
  return result;
}
export async function reportGeneralVisualReady(facts: Record<string, unknown>, error?: string): Promise<void> {
  const context=getGeneralVisualContext();
  if (Object.keys(facts).some(key=>!factsKeys.has(key))) throw new Error('Unknown visual observation field');
  const actualFacts={...facts};
  let actualError=error ?? null;
  let native:NativeBridge|undefined;
  if(context.transport==='packaged_native') {
    native=(window as unknown as {__TAURI_INTERNALS__?:NativeBridge}).__TAURI_INTERNALS__;
    if (!native) throw new Error('Isolated Native observation transport is absent');
    nativeAclProbe??=probeNativeAcl(native);
    const acl=await nativeAclProbe; actualFacts.nativeAcl=acl;
    if(!acl.coreDenied||!acl.businessUnknown)actualError=actualError??'Isolated Native ACL or business-handler exclusion probe failed';
  }
  bounded(actualFacts);
  const result=Object.freeze({context,facts:actualFacts,error:actualError});
  const encoded=JSON.stringify(result);
  if (new TextEncoder().encode(encoded).byteLength > 65536 || (actualError?.length ?? 0) > 2048) throw new Error('Visual observation exceeds limit');
  window.__GENERAL_VISUAL_RESULT__=result;
  if (context.transport === 'real_react') return;
  await native!.invoke('general_visual_observed',{observation:result});
}
