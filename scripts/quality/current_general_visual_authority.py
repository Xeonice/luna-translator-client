"""Code-owned General source, case and renderer authority; never product acceptance.

Only the user-approved source closure and finite test entry are accepted. A caller
cannot supply another root, URL, script, state list, mask or comparison threshold.
The historical Chromium counterexamples are observer diagnostics, not deleted debt.
"""
from __future__ import annotations
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PREFIX = 'docs/design/first-product-v1/candidates/general-state-evidence-20261108/'
APPROVAL = 'docs/development/design-approval-general-states-20261108.json'
APPROVAL_SHA = '6e1694583d1c12a7597c9b00cf91dd21154a04f6f463fc4a9aa48a5042a6cb8a'
PROMOTION = PREFIX + 'promotion.json'
PROMOTION_SHA = '345205c37d1b77df870b429efbd84c6811c34f1416bbd0b1cc8cac1e81547b84'
RENDER_FILES = ('settings.html','state-gap.css','state-gap.js','assets/icons.js','assets/material-tokens.css','source/settings.css','source/settings.js')
STATE_NAMES = ('normal','empty','loading','error','disabled','long-text','light','narrow','zoom-200','keyboard-focus-escape','scope-global','scope-game','restore-inheritance','save-failed')
STATE_IDS = tuple('all-page-settings-general-' + name for name in STATE_NAMES)
CANVASES = tuple({'canvas_id':f'{w}x{h}-{theme}-z{zoom}','width':w,'height':h,'theme':theme,'zoom':zoom}
                 for w,h in ((1000,720),(760,560)) for theme in ('dark','light') for zoom in (1,2))
ENTRY = 'apps/desktop/tests/native/general/entry.tsx'
ENTRY_FILES = (ENTRY, 'apps/desktop/tests/native/general/general-port.ts', 'apps/desktop/tests/native/general/state-contract.ts',
               'apps/desktop/tests/native/general/fixture-data.json', 'apps/desktop/tests/native/general/native-context.ts')
REFERENCE_REPLAY = 'apps/desktop/tests/native/general/reference-replay.js'
SOURCE_BUILD_HELPER = 'scripts/report/general_visual_assets.mjs'
HOST = 'scripts/report/general_visual_webkit_host.swift'
DRIVER = 'scripts/report/general_visual_webkit.py'
CONTEXT = 'user-approved-native-content-transparent-body-system-chrome'
SCOPE = 'settings:general/controlled_visual_only'
LAYERS = ('real_react', 'packaged_native')


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def canonical(value) -> bytes:
    return (json.dumps(value,ensure_ascii=False,sort_keys=True,separators=(',', ':'))+'\n').encode()


def read(relative: str, root: Path = ROOT) -> bytes:
    if not isinstance(relative,str) or not relative or '\\' in relative or any(c in relative for c in ':?#\0'):
        raise ValueError('General finite relative path required')
    p = Path(relative)
    if p.is_absolute() or any(v in ('', '.', '..') for v in p.parts):
        raise ValueError('General path escapes source closure')
    current = root.resolve()
    for segment in p.parts:
        current /= segment
        if current.is_symlink():
            raise ValueError('General source symlink forbidden')
    if not current.is_file():
        raise ValueError('General source input absent: '+relative)
    return current.read_bytes()


def source_inputs(root: Path = ROOT) -> dict:
    approval_bytes, promotion_bytes = read(APPROVAL,root), read(PROMOTION,root)
    if digest(approval_bytes) != APPROVAL_SHA or digest(promotion_bytes) != PROMOTION_SHA:
        raise ValueError('User-approved General source record differs')
    approval, promotion = json.loads(approval_bytes), json.loads(promotion_bytes)
    if approval.get('status') != 'approved_by_user' or approval.get('native_verified') is not False or approval.get('product_accepted') is not False:
        raise ValueError('Source approval is not a scoped user-source decision')
    rows = {APPROVAL:{'sha256':APPROVAL_SHA,'bytes':len(approval_bytes)},PROMOTION:{'sha256':PROMOTION_SHA,'bytes':len(promotion_bytes)}}
    for name in RENDER_FILES:
        relative = PREFIX + name
        relocation = [r for r in promotion['relocation'] if r['promoted']['path'] == relative]
        if len(relocation) != 1:
            raise ValueError('Source relocation is not exact')
        row = relocation[0]
        if row['transformation'] != 'verbatim' or row['captured']['sha256'] != row['promoted']['sha256'] or row['captured']['bytes'] != row['promoted']['bytes']:
            raise ValueError('General source content transform was not approved')
        data = read(relative,root)
        if len(data) != row['promoted']['bytes'] or digest(data) != row['promoted']['sha256']:
            raise ValueError('Approved General render input changed: '+relative)
        rows[relative] = {'sha256':digest(data),'bytes':len(data)}
    return rows


def reference_inputs(root: Path = ROOT, layer: str = 'real_react') -> dict:
    # Producer code is separate from the eventual product source epoch. References
    # cannot recursively require their own final product/receipt to exist.
    if layer not in LAYERS: raise ValueError('Unknown General reference renderer')
    rows = source_inputs(root)
    for relative in (REFERENCE_REPLAY,'apps/desktop/tests/native/general/native-context.ts',SOURCE_BUILD_HELPER,HOST,DRIVER,
                     'scripts/report/capture_general_visual_references.mjs','scripts/report/verify_general_visual_react.mjs',
                     'apps/desktop/tests/native/general/fixture-data.json'):
        data=read(relative,root); rows[relative]={'sha256':digest(data),'bytes':len(data)}
    if layer == 'packaged_native':
        for relative in ('apps/desktop/src-tauri/src/main.rs','apps/desktop/src-tauri/src/visual_verification.rs',
                         'apps/desktop/src-tauri/tauri.visual.conf.json','apps/desktop/src-tauri/Cargo.toml','apps/desktop/src-tauri/build.rs',
                         'Cargo.toml','Cargo.lock','apps/desktop/vite.config.ts','apps/desktop/general-visual.html',
                         'apps/desktop/package.json','apps/desktop/package-lock.json',
                         'scripts/report/verify_general_native_visual.py','scripts/quality/runtime_observer.swift',
                         'scripts/quality/general_native_display_probe.swift','scripts/quality/current_general_native_display.py'):
            data=read(relative,root); rows[relative]={'sha256':digest(data),'bytes':len(data)}
    return rows


def candidate_inputs(root: Path = ROOT) -> dict:
    rows={}
    for directory in ('apps/desktop/src',):
        for item in sorted((root/directory).rglob('*')):
            if item.is_symlink(): raise ValueError('Production source symlink forbidden')
            if item.is_file():
                rel=item.relative_to(root).as_posix();data=read(rel,root);rows[rel]={'sha256':digest(data),'bytes':len(data)}
    for relative in (*ENTRY_FILES,REFERENCE_REPLAY,SOURCE_BUILD_HELPER,'apps/desktop/general-visual.html','apps/desktop/vite.config.ts',
                     'apps/desktop/package.json','apps/desktop/package-lock.json','package.json','package-lock.json',HOST,DRIVER):
        data=read(relative,root);rows[relative]={'sha256':digest(data),'bytes':len(data)}
    return rows


def closure_sha(rows: dict) -> str:
    return digest(canonical(rows))


def canvas(canvas_id: str) -> dict:
    found = [row for row in CANVASES if row['canvas_id'] == canvas_id]
    if len(found)!=1: raise ValueError('Unknown General canvas')
    return dict(found[0])


def validate_case(state_id: str, canvas_id: str, subcase: str) -> dict:
    if state_id not in STATE_IDS: raise ValueError('Unknown General state')
    data=canvas(canvas_id)
    if subcase != 'primary' and not (subcase=='initial' and state_id==STATE_IDS[2]) and not (subcase=='menu-open' and state_id==STATE_IDS[9]) and not (subcase=='content-end' and state_id==STATE_IDS[0] and data['zoom']==2):
        raise ValueError('Unknown General subcase')
    return {'state_id':state_id,'subcase':subcase,**data}


def capture_cases() -> tuple[dict,...]:
    values=[]
    for c in CANVASES:
        values.append(validate_case(STATE_IDS[0],c['canvas_id'],'primary'))
        if c['zoom']==2: values.append(validate_case(STATE_IDS[0],c['canvas_id'],'content-end'))
    for name in STATE_NAMES[1:]:
        cid={'light':'1000x720-light-z1','narrow':'760x560-dark-z1','zoom-200':'1000x720-dark-z2'}.get(name,'1000x720-dark-z1')
        values.append(validate_case('all-page-settings-general-'+name,cid,'primary'))
        if name=='loading':values.append(validate_case(STATE_IDS[2],cid,'initial'))
        if name=='keyboard-focus-escape':values.append(validate_case(STATE_IDS[9],cid,'menu-open'))
    assert len(values)==27 and set(r['state_id'] for r in values)==set(STATE_IDS)
    return tuple(values)


def case_key(row: dict) -> str:
    validate_case(row['state_id'],row['canvas_id'],row['subcase'])
    return row['state_id'].replace('all-page-settings-general-','')+'--'+row['canvas_id']+'--'+row['subcase']


def context(row: dict, side: str, nonce: str, transport: str='real_react') -> dict:
    expected=validate_case(row['state_id'],row['canvas_id'],row['subcase'])
    if row != expected: raise ValueError('General case is not the canonical canvas representation')
    if side not in ('reference','candidate') or transport not in LAYERS or len(nonce)!=32 or any(c not in '0123456789abcdef' for c in nonce):
        raise ValueError('Invalid General observation context')
    return {'schemaVersion':1,'kind':'general-native-visual-context/v1','transport':transport,'side':side,'stateId':row['state_id'],
            'subcase':row['subcase'],'canvasId':row['canvas_id'],'nonce':nonce,'width':row['width'],'height':row['height'],'theme':row['theme'],'zoom':row['zoom']}


def recipes() -> dict:
    return {state:{'entry':PREFIX+'settings.html','render_source':'independent-user-approved-general-html','context':CONTEXT,
                   'state_id':state,'cases':[dict(r) for r in capture_cases() if r['state_id']==state],
                   'fixture_visual_only':True,'os_runtime_verified':False,'storage_runtime_verified':False,'game_tests_executed':False}
            for state in STATE_IDS}


def authority(root: Path = ROOT) -> dict:
    rows=source_inputs(root)
    return {'kind':'general-visual-source-authority/v1','scope':SCOPE,'approval':{'path':APPROVAL,'sha256':APPROVAL_SHA},
            'promotion':{'path':PROMOTION,'sha256':PROMOTION_SHA},'source_inputs':rows,'source_closure_sha256':closure_sha(rows),
            'state_ids':list(STATE_IDS),'cases':list(capture_cases()),'context':CONTEXT,'capture_scope':'whole_client',
            'rgba_threshold':0,'mask':False,'crop':False,'resample':False,'product_accepted':False}

if __name__=='__main__':
    print(json.dumps(authority(),ensure_ascii=False,indent=2))


def expected_mapping(root: Path = ROOT) -> dict:
    """Exact OpenSpec metadata contract; a new arbitrary candidate cannot register."""
    value=authority(root)
    return {'schema_version':1,'kind':'general-visual-source-mapping/v1','scope':SCOPE,'entry':PREFIX+'settings.html',
            'approval':value['approval'],'promotion':value['promotion'],'source_closure_sha256':value['source_closure_sha256'],
            'context':CONTEXT,'state_ids':list(STATE_IDS),'capture_scope':'whole_client','cases':list(capture_cases()),
            'rgba_threshold':0,'mask':False,'crop':False,'resample':False,
            'fixture_visual_only':True,'game_tests_executed':False,'os_runtime_verified':False,'storage_runtime_verified':False}


def design_recipe(name: str) -> list[dict]:
    """Ordinary fixed source-DOM actions for the General route, never product DOM."""
    name=name.replace('_','-')
    if name not in STATE_NAMES: raise ValueError('Unknown General design state')
    source_state={'loading':'loading','error':'read-error','save-failed':'save-failed','empty':'empty'}.get(name,'normal')
    theme='light' if name=='light' else 'dark';zoom='2' if name=='zoom-200' else '1'
    size=(760,560) if name=='narrow' else (1000,720)
    result=[{'action':'set_viewport','width':size[0],'height':size[1]},
            {'action':'navigate','path':PREFIX+'settings.html','query':{'theme':theme,'zoom':zoom,'state':source_state,'context':'native-proposal'}}]
    def click(selector):return {'action':'click','selector':selector}
    if name in ('scope-game','restore-inheritance','long-text'):result.extend([click('#scope-button'),click('[data-scope="game"]')])
    if name=='disabled':result.extend([click('#pos-switch'),click('#seg-switch')])
    if name=='restore-inheritance':result.extend([click('#ruby-switch'),click('#save'),{'action':'assert_property','selector':'#save','name':'disabled','value':True},click('#restore-inheritance')])
    if name=='keyboard-focus-escape':result.extend([click('#scope-button'),{'action':'press','key':'Home'},{'action':'press','key':'End'},{'action':'press','key':'Escape'}])
    if name=='long-text':result.append({'action':'evaluate','script':"for(const selector of ['#scope-value','[data-scope=game]','.status-scope'])document.querySelector(selector).textContent='仅此游戏 · サクラノ詩 Re:Graphic 10th Anniversary edition — 長いゲーム名と保存された読書設定の表示確認';"})
    result.extend([{'action':'assert_visible','selector':'.window'},
                   {'action':'assert_design_predicate','expression':'window.SettingsStructureCandidate !== undefined && window.GeneralStateGapCandidate !== undefined'},
                   {'action':'assert_design_predicate','expression':"document.documentElement.dataset.context === 'native-proposal'"},
                   {'action':'assert_design_predicate','expression':f"document.documentElement.dataset.theme === '{theme}' && Number(getComputedStyle(document.documentElement).zoom) === {zoom}"}])
    if name=='keyboard-focus-escape':result.append({'action':'assert_focused','selector':'#scope-button'})
    if name in ('loading','error','save-failed'):result.append({'action':'assert_visible','selector':'#state-notice'})
    if name in ('scope-game','restore-inheritance','long-text'):result.append({'action':'assert_design_predicate','expression':"window.SettingsStructureCandidate.state.scope === 'game'"})
    return result


def metadata_gaps(root: Path, metadata: dict) -> list[str]:
    """Executed requirement-level mapping check, independent from AST success."""
    try:
        checks={'general-visual-contract-tests','product-general-visual-react-tests','product-general-visual-native-tests'}
        rows=[row for row in metadata.get('requirements',[]) if isinstance(row,dict) and checks & set(row.get('checks',[]))]
        if len(rows)!=1: raise ValueError('Exactly one General visual requirement with its own source mapping is required')
        value=rows[0].get('general_visual_source')
        if canonical(value)!=canonical(expected_mapping(root)):
            raise ValueError('Requirement General source/context/14-state/27-case mapping differs from code-owned approved authority')
        return []
    except (OSError,ValueError,TypeError,KeyError) as error:
        return ['General visual source metadata invalid: '+str(error)]
