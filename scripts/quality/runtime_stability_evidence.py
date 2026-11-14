"""Read-only recomputation of a real stability run; producer/user verdicts are ignored."""
from __future__ import annotations
import base64
from datetime import datetime
import hashlib
import json
from pathlib import Path
import re
import struct
import unicodedata
from gate_core import digest, safe_path
from lifecycle_impact import unique_json, tree_hash

CASE_IDS=('SR6-IDENTITY','SR6-COLD-DISCOVERY','SR6-CONTINUOUS-TEXT','SR6-JAPANESE-LEXICAL','SR6-PREVIEW-CHANGED-FRAMES','SR6-PRODUCER-ISOLATION','SR6-FROZEN-WORDCARD','SR6-NATURAL-EXIT-SWITCH','SR6-QUEUE-PROGRESS','SR6-REAL-SLEEP-WAKE')

class EvidenceUnavailable(ValueError): pass

def need(condition,message):
    if not condition: raise EvidenceUnavailable(message)

def norm(value):
    return ''.join(c for c in unicodedata.normalize('NFKC',value) if not c.isspace())

def inspect_body(session):
    raw={r['event_id']:r for r in session['raw']};payloads={r['event_id']:base64.b64decode(r['base64'],validate=True) for r in session['payloads']}
    records=[]
    for b in session['body']:
        encoded=b['source_text'].encode()
        if hashlib.sha256(encoded).hexdigest()!=b['source_sha256'] or len(encoded)!=b['source_byte_length']: raise ValueError('actual source bytes/hash differ')
        refs=sorted((r for r in session['refs'] if r['unit_id']==b['unit_id'] and r['text_revision']==b['text_revision']),key=lambda r:r['ref_ordinal'])
        need(refs,'body has no actual raw provenance')
        chunks=[]
        for ref in refs:
            e=raw[ref['event_id']];p=payloads.get(ref['event_id']);need(p is not None,'source payload unavailable')
            if e['integrity_state']!='intact' or hashlib.sha256(p).hexdigest()!=e['payload_sha256'] or len(p)!=e['payload_byte_length']: raise ValueError('raw payload integrity mismatch')
            start,end=ref['byte_start'],ref['byte_end']
            if not 0<=start<=end<=len(p): raise ValueError('source ref outside actual payload')
            p[:start].decode('utf-8');p[start:end].decode('utf-8');p[end:].decode('utf-8');chunks.append(p[start:end])
        if b''.join(chunks)!=encoded: raise ValueError('actual raw spans do not reconstruct current source')
        records.append(b)
    return records

def canonical_u64(value):
    if not isinstance(value,str) or not re.fullmatch('0|[1-9][0-9]{0,19}',value) or int(value)>2**64-1:raise ValueError('noncanonical actual uint64 identity')
    return int(value)

def stream_key(stream):
    if type(stream['pid']) is not int or not 0<stream['pid']<=2**32-1:raise ValueError('invalid actual stream PID')
    return (stream['pid'],*[canonical_u64(stream[k]) for k in ('address','context','context2')])

def inspect_host_tail(session,stdout,final):
    s=session['session'];sid=s['session_id'];pid=s['windows_pid'];target=final['target']
    if final['session_id']!=sid or (target['pid'],target['full_image_path'],target['creation_filetime'],target['architecture'])!=(pid,s['target_exe_path'],s['process_created_u64'],s['architecture']):raise ValueError('Host final SID/target identity differs from durable session')
    callbacks=[]
    controls={'ready','response','log','hook_inserted','injection_finished','stream_created','stream_destroyed','process_connected','process_disconnected'}
    for row in stdout:
        if row.get('v')!=2:raise ValueError('actual Host protocol version differs')
        event=row.get('event')
        if event=='text':
            key=stream_key(row['stream'])
            if key[0]!=pid:raise ValueError('Host text belongs to different target PID')
            callbacks.append((*key,row['hook_code'],row['hook_name'],row['text']))
        elif event not in controls:raise ValueError('unknown/error/overflow Host event cannot be filtered from tail')
        elif event=='process_disconnected' and row['pid']!=pid:
            # This is the one non-target PID event the real client ignores.
            continue
        elif 'stream' in row and stream_key(row['stream'])[0]!=pid or 'pid' in row and row['pid']!=pid:raise ValueError('non-target Host event cannot be filtered from tail')
    need(callbacks,'no actual Host text callbacks')
    if final['text_events']!=len(callbacks):raise ValueError('Host callbacks/final text count differs')
    history=[];keys=set()
    for stream in final['streams']:
        key=stream_key(stream['id'])
        if key in keys:raise ValueError('duplicate final stream identity')
        keys.add(key)
        # Core JSONL has no sequence. The real bounded client history is the
        # independent bridge to ClientEvent.sequence, never the stdout ordinal.
        need(stream['text_events']==len(stream['history']),'bounded client history cannot prove full Host wire tail')
        last=0
        for line in stream['history']:
            seq=line['sequence']
            if type(seq) is not int or not last<seq<=final['sequence']:raise ValueError('actual client history sequence invalid/reordered')
            last=seq;history.append((seq,(*key,stream['hook_code'],stream['hook_name'],line['text'])))
    history.sort(key=lambda x:x[0])
    if len({n for n,_ in history})!=len(history) or callbacks!=[r for _,r in history]:raise ValueError('Host callback order/complete stream metadata differs from actual client history')
    sources={r['source_id']:r for r in session['sources']}
    if len(sources)!=len(session['sources']):raise ValueError('duplicate durable source identity')
    payloads={r['event_id']:base64.b64decode(r['base64'],validate=True) for r in session['payloads']}
    if len(payloads)!=len(session['payloads']):raise ValueError('duplicate durable payload identity')
    raw=session['raw'];ids={r['event_id']:r for r in raw}
    if len(ids)!=len(raw) or [r['ingest_sequence'] for r in raw]!=list(range(1,len(raw)+1)):raise ValueError('durable raw ingest order/identity incomplete')
    retained=[];last=0
    for row in raw:
        if row['session_id']!=sid or row['integrity_state']!='intact':raise ValueError('durable raw SID/integrity differs')
        if row['event_kind']!='text':continue
        seq=canonical_u64(row['wire_sequence_u64'])
        if not last<seq or row['event_id']!=f'{sid}:{seq}':raise ValueError('durable text wire/order/event SID differs')
        last=seq;c=sources[row['source_id']];ext=json.loads(c['identity_extensions_json'])
        key=stream_key({'pid':ext['pid'],'address':c['hook_address_u64'],'context':c['context_u64'],'context2':c['subcontext_u64']})
        h=hashlib.sha256(f'{sid}:{key[0]}:{key[1]}:{key[2]}:{key[3]}'.encode()).hexdigest()
        if key[0]!=pid or c['session_id']!=sid or c['host_instance_id']!=s['broker_instance_id'] or c['stream_identity_sha256']!=h or c['source_id']!='source-'+h:raise ValueError('durable source stream/target/SID/Host identity differs')
        data=payloads.get(row['event_id']);need(data is not None,'actual Host payload not retained')
        if len(data)!=row['payload_byte_length'] or hashlib.sha256(data).hexdigest()!=row['payload_sha256']:raise ValueError('durable Host payload bytes/hash differs')
        retained.append((seq,(*key,c['hook_code'],c['hook_name'],data.decode('utf-8'))))
    if retained!=history:raise ValueError('Host ordered callbacks/actual client wire differ from durable raw journal')
    revisions=session['revisions'];revision_keys={(b['unit_id'],b['text_revision']):b for b in revisions}
    if len(revision_keys)!=len(revisions):raise ValueError('duplicate durable source revision')
    selections={r['selection_revision']:r for r in session['selections']}
    for ref in session['refs']:
        b=revision_keys[(ref['unit_id'],ref['text_revision'])];row=ids[ref['event_id']];selection=selections[b['selection_revision']]
        if ref['session_id']!=sid or b['session_id']!=sid or selection['session_id']!=sid or selection['source_id']!=row['source_id']:raise ValueError('source ref crosses actual SID/selection/stream')
    inspect_body({**session,'body':revisions})
    return len(callbacks)

def inspect_lexical(session,body):
    linked=[r for r in session['lexical'] if r['unit_id']==body['unit_id'] and r['text_revision']==body['text_revision']]
    need(linked,'no actual fixed-version lexical snapshot')
    l=linked[-1];tokens=sorted((r for r in session['tokens'] if r['lexical_id']==l['lexical_id']),key=lambda r:r['token_index']);b=body['source_text'].encode()
    if l['source_sha256']!=body['source_sha256'] or len(tokens)!=l['token_count']: raise ValueError('lexical version/count differs from fixed source')
    at=0
    for i,t in enumerate(tokens):
        if t['token_index']!=i or t['byte_start']!=at or t['byte_end']<=at: raise ValueError('token spans overlap, gap or reorder')
        part=b[at:t['byte_end']].decode()
        if part!=t['surface']: raise ValueError('token surface differs from UTF8 bytes')
        at=t['byte_end']
    if at!=len(b): raise ValueError('tokens do not reconstruct source')
    need(any(t['reading_kana'] and t['is_known'] and t['token_kind']=='word' for t in tokens),'no real known word/kana')
    return tokens

def read_jsonl(path):
    data=path.read_bytes()
    # A snapshot may end in an in-flight append. It is not a completed record;
    # later health/count closure must account for every required fact.
    lines=data.split(b'\n')[:-1]
    return [json.loads(line) for line in lines if line]

def load_trace_archive(directory,files,nonce):
    need(files,'missing immutable observation archive')
    rows=[]
    for filename,sha in sorted(files.items()):
        p=safe_path(directory,filename)
        if digest(p)!=sha: raise ValueError('observation archive hash mismatch')
        data=p.read_bytes()
        if not data.endswith(b'\n'):raise ValueError('observation archive has incomplete tail record')
        for offset,row in enumerate(read_jsonl(p)):
            if row.get('execution_nonce')!=nonce: raise ValueError('old observation nonce')
            if p.name!=f"segment-{row['segment_index']:08}.jsonl":raise ValueError('archive filename/segment identity differs')
            rows.append({**row,'_physical_order':(row['segment_index'],offset)})
    return rows

def trace_structure(rows,pid,start):
    begins=[r for r in rows if r['kind']=='observation_begin'];need(len(begins)==1,'missing/ambiguous current observer begin')
    begin=begins[0];run=begin['run_id']
    if begin['pid']!=pid or begin['initialized_at_ms']<start or any(r['run_id']!=run for r in rows): raise ValueError('old/mixed observer PID/start/run')
    facts=[r for r in rows if r['kind']=='runtime_fact'];health=[r for r in rows if r['kind']=='observation_health'];need(health,'missing actual metadata health')
    if len({r['attempt_sequence'] for r in facts})!=len(facts): raise ValueError('duplicate/replaced observation attempt')
    for h in health:
        if any(h.get(k)!=0 for k in ('dropped','invalid','write_failures')): raise ValueError('observer loss/write failure; cannot prove interval')
        if h['rotation']!=h['segment_index']: raise ValueError('observer segment identity differs')
        actual=[r for r in facts if r['_physical_order']<h['_physical_order']]
        # These counts belong to the writer. Producer counters may transiently
        # lag; such a health is retained, but cannot close an interval.
        if h['written_facts']!=len(actual) or h['processed']!=h['written_facts']: raise ValueError('missing written records or failed write; original segment count does not close')
        if actual and (h['max_observed_attempt']!=max(r['attempt_sequence'] for r in actual) or h['last_written_attempt']!=actual[-1]['attempt_sequence']): raise ValueError('writer maximum/last attempt differs from actual record set')
        if not actual and (h['max_observed_attempt']!=0 or h['last_written_attempt']!=0):raise ValueError('zero-fact health has nonzero writer attempt watermark')
    need(set(range(begin['segment_index'],max(r['segment_index'] for r in rows)+1)) <= {r['segment_index'] for r in rows},'missing observer rotation segment')
    return facts,health

def stable_health(facts,h):
    actual=[r for r in facts if r['_physical_order']<h['_physical_order']]
    count=len(actual)
    # Check the actual ID set, not a fabricated prefix inferred from max().
    # Out-of-order queue insertion is allowed; unresolved holes are not.
    return all(type(h.get(k)) is int and h[k]==count for k in ('attempted','enqueued','processed','written_facts')) and {r['attempt_sequence'] for r in actual}==set(range(1,count+1))

def trace_interval(rows,pid,start,boundary_ms,step_id):
    facts,health=trace_structure(rows,pid,start)
    eligible=[h for h in health if h['at_ms']>=boundary_ms and stable_health(facts,h)]
    need(len(eligible)>=2,'observation interval lacks two subsequent stable health records')
    b=eligible[-1];a=next((h for h in reversed(eligible[:-1]) if h['at_ms']<b['at_ms']),None)
    need(a is not None,'observation interval lacks two subsequent stable health records')
    if any(f['observed_at_ms']<=boundary_ms and f['_physical_order']>b['_physical_order'] for f in facts):raise ValueError('late unaccounted fact inside observation interval')
    return {'step_id':step_id,'boundary_ms':boundary_ms,'opening_health':list(a['_physical_order']),'closing_health':list(b['_physical_order'])}

def validate_trace_interval(rows,facts,interval,pid,start):
    lookup={r['_physical_order']:r for r in rows}
    a=lookup.get(tuple(interval['opening_health']));b=lookup.get(tuple(interval['closing_health']))
    need(a is not None and b is not None,'observation interval health missing')
    if a['kind']!='observation_health' or b['kind']!='observation_health' or not a['_physical_order']<b['_physical_order'] or not start<=interval['boundary_ms']<=a['at_ms']<b['at_ms']:raise ValueError('observation interval health does not follow actual boundary')
    need(stable_health(facts,a) and stable_health(facts,b),'observation interval has pending/unaccounted tail attempts')
    if any(f['observed_at_ms']<=interval['boundary_ms'] and f['_physical_order']>b['_physical_order'] for f in facts):raise ValueError('unaccounted tail fact inside observation interval')
    return b

def inspect_trace(directory,manifest,nonce,pid,start):
    rows=load_trace_archive(directory,manifest.get('trace_files',{}),nonce)
    facts,health=trace_structure(rows,pid,start)
    # A cold checkpoint may legitimately precede every business observation.
    # A whole packaged game run cannot use that empty interval as its evidence.
    need(facts,'whole runtime lacks actual metadata facts')
    intervals=manifest.get('observation_intervals',[]);need(intervals,'missing closed observation intervals')
    ids=[i['step_id'] for i in intervals]
    if len(ids)!=len(set(ids)) or ids[-1]!='final':raise ValueError('observation interval inventory invalid')
    expected=[r['step_id'] for r in manifest.get('steps',[])]+['final']
    if ids!=expected:raise ValueError('every required step needs its own closed observation interval')
    boundaries={r['step_id']:r['collected_at_ms'] for r in manifest.get('steps',[])}
    boundaries['final']=manifest['observation_completed_at_ms']
    if boundaries['final']<max(boundaries.values()) or manifest.get('completed_at_ms',boundaries['final'])<boundaries['final']:raise ValueError('final observation interval excludes step boundary')
    for interval in intervals:
        if interval['boundary_ms']!=boundaries[interval['step_id']]:raise ValueError('observation interval differs from actual collected boundary')
        last=validate_trace_interval(rows,facts,interval,pid,start)
    if rows[-1]['_physical_order']!=last['_physical_order']:raise ValueError('unaccounted observation tail after final health')
    live=load_trace_archive(directory,manifest.get('live_trace_files',{}),nonce)
    prefix=[r for r in live if r['_physical_order']<=last['_physical_order']]
    if prefix!=rows:raise ValueError('closed observation archive differs from original live prefix')
    live_paths={Path(p).name:safe_path(directory,p) for p in manifest['live_trace_files']}
    for name in manifest['trace_files']:
        frozen=safe_path(directory,name);original=live_paths[frozen.name].read_bytes()
        segment=int(frozen.stem.split('-')[1]);limit=last['_physical_order']
        expected=b''.join(original.splitlines(keepends=True)[:limit[1]+1]) if segment==limit[0] else original
        if frozen.read_bytes()!=expected:raise ValueError('closed observation bytes differ from original live prefix')
    if any(r['kind']=='runtime_fact' and r['observed_at_ms']<=boundaries['final'] for r in live[len(prefix):]):raise ValueError('live archive contains unaccounted interval tail fact')
    if manifest.get('archive_errors'): raise ValueError('passive ledger archive failed')
    validate_frame_metadata(facts)
    return sorted(facts,key=lambda r:(r['observed_at_ms'],r['attempt_sequence'])),last

def validate_frame_metadata(facts):
    services={};workers={}
    for row in facts:
        f=row['fact']
        if f['kind']=='collector_frame' and f['accepted']:
            key=tuple(f[k] for k in ('source_id','source_epoch','frame_epoch','capture_id','worker_sequence'))
            value=tuple(f[k] for k in ('target_candidate_id','host_pid','host_start','window_id','captured_at_ms','content_hash','width','height','encoded_len'))
            if key in workers and workers[key]!=value:raise ValueError('accepted worker sequence identity/payload changed')
            workers[key]=value
        if f['kind']!='projection' or not f.get('frame'):continue
        g=f['frame'];projection=f['projection'];key=tuple(g[k] for k in ('lease_id','generation','service_sequence'))
        if any(g[k]!=projection[k] for k in ('lease_id','generation','target_identity_hash','captured_at_ms')):raise ValueError('delivered frame/projection identity differs')
        if not re.fullmatch('[0-9a-f]{64}',g['content_hash']) or any(type(g[k]) is not int or g[k]<=0 for k in ('width','height','encoded_len')):raise ValueError('not a real bounded image frame')
        value=tuple(g[k] for k in ('target_identity_hash','captured_at_ms','content_hash','width','height','encoded_len'))
        if key in services and services[key]!=value:raise ValueError('same service sequence replaced frame/time')
        services[key]=value
        # Ordering of independent enqueues is not commit order. Validate an
        # actual delivery against its own read cursor, never sort worker seq as
        # though it were a service sequence or use log arrival as linearization.
        after=f.get('requested_after_sequence')
        if f['stage']=='server_delivered' and after is not None and int(g['service_sequence'])<=int(after):raise ValueError('delivered frame is not beyond requested service cursor')
    return services,workers

def window(snapshot,role):
    wid=snapshot['action']['windows'].get(role);need(wid is not None,'missing actual '+role+' window selection')
    matches=[r for r in snapshot['os']['windows'] if r['window_id']==wid]
    need(len(matches)==1 and matches[0].get('on_screen') is True,'selected '+role+' window unavailable')
    expected=snapshot['action']['game_host_pid'] if role=='game' else snapshot['pid']
    if matches[0]['pid']!=expected:raise ValueError(role+' window belongs to a different process')
    return matches[0]

def visible_text(snapshot,role,text):
    w=window(snapshot,role);capture=w.get('capture',{})
    need(capture.get('png') and capture.get('ocr'),'independent '+role+' screenshot/OCR unavailable')
    words=''.join(r['text'] for r in capture['ocr'])
    if norm(text) not in norm(words): raise ValueError('actual '+role+' OCR differs from fixed current source')
    return w

def preview_text(snapshot,text):
    w=window(snapshot,'preview');crops=w.get('capture',{}).get('preview_crops',[])
    need(len(crops)==1,'unique actual AX preview image/crop unavailable')
    if '目标窗口当前画面' not in crops[0]['description']: raise ValueError('preview is not visibly current')
    need(crops[0].get('png') and crops[0].get('ocr'),'preview crop OCR unavailable')
    if norm(text) not in norm(''.join(r['text'] for r in crops[0]['ocr'])): raise ValueError('actual visible preview image differs from current game sentence')
    return crops[0]

def lsof_entries(text):
    out=[];row={}
    for line in text.splitlines():
        if not line:continue
        if line[0]=='f':
            if row:out.append(row)
            row={'fd':line[1:]}
        elif line[0]=='i':row['inode']=line[1:]
        elif line[0]=='n':row['name']=line[1:]
    if row:out.append(row)
    return out

def session_for(snapshot,game):
    def matches(s):return all(s[k]==game[v] for k,v in [('environment_key','environment_key'),('target_exe_path','target_exe_path'),('architecture','architecture')])
    rows=[r for r in snapshot['database']['sessions'] if matches(r['session'])]
    need(len(rows)==1,'exact new target session missing/ambiguous for '+game['id'])
    s=rows[0]['session']
    expected='\0'.join(str(s[k]) for k in ('environment_key','target_exe_path','windows_pid','process_created_u64','architecture'))
    if hashlib.sha256(expected.encode()).hexdigest()!=s['target_identity_sha256']:raise ValueError('full Windows target identity hash differs')
    return rows[0]

def new_frames(facts,snapshot,after=0):
    end=snapshot['os']['observed_at_ms'];pid=snapshot['action']['game_host_pid']
    accepted=[r['fact'] for r in facts if after<int(r['observed_at_ms'])<=end and r['fact']['kind']=='collector_frame' and r['fact']['accepted'] and r['fact']['host_pid']==pid]
    committed=[r['fact'] for r in facts if after<int(r['observed_at_ms'])<=end and r['fact']['kind']=='projection' and r['fact']['stage']=='service_committed' and r['fact'].get('frame')]
    delivered=[r['fact'] for r in facts if after<int(r['observed_at_ms'])<=end and r['fact']['kind']=='projection' and r['fact']['stage']=='server_delivered' and r['fact'].get('frame')]
    joined=[]
    for f in accepted:
        physical=hashlib.sha256(f"{f['target_candidate_id']}\0{f['host_pid']}\0{f['host_start']}\0{f['window_id']}".encode()).hexdigest()
        for c in committed:
            g=c['frame']
            if g['content_hash']==f['content_hash'] and int(g['captured_at_ms'])==f['captured_at_ms'] and g['target_identity_hash']==physical and g['width']==f['width'] and g['height']==f['height'] and g['encoded_len']>0:
                if any(d['frame']==g for d in delivered):joined.append((f,g))
    need(joined,'no actual guard -> service commit -> server delivery frame chain')
    return joined

def power_sleep_wake(text,start,end):
    events=[]
    for line in text.splitlines():
        match=re.match(r'(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) ([+-]\d{4})\s+(Sleep|Wake)\s+(.*)',line)
        if not match:continue
        at=int(datetime.strptime(match[1]+' '+match[2],'%Y-%m-%d %H:%M:%S %z').timestamp()*1000)
        if start<=at<=end and (match[3]=='Sleep' and 'Entering Sleep state' in match[4] or match[3]=='Wake' and 'Wake from' in match[4]):events.append((at,match[3]))
    return any(a[1]=='Sleep' and b[1]=='Wake' and a[0]<b[0] for a in events for b in events)

def validate_run(root,directory,nonce,source):
    from runtime_stability import GAMES,plan_steps,load_inputs
    cases={i:{'id':i,'status':'not_run','assertion_count':0,'errors':[]} for i in CASE_IDS};errors=[];steps={};facts=[];manifest={};x={}
    def check(identity,fn):
        row=cases[identity]
        try:
            count=fn();row.update(status='passed',assertion_count=count)
        except EvidenceUnavailable as e:row.update(status='blocked',errors=[str(e)])
        except (OSError,ValueError,KeyError,TypeError,UnicodeError) as e:row.update(status='failed',errors=[str(e)])
    try:
        manifest=unique_json(directory/'collection.json');x=load_inputs(directory/'inputs.json',root)
        if manifest['kind']!='real_packaged_stability_collection' or manifest['execution_nonce']!=nonce or manifest['source_tree_sha256']!=source:raise ValueError('old nonce/source or non-runtime collection')
        if manifest['inputs_sha256']!=digest(directory/'inputs.json') or manifest['source_tree_sha256_after']!=source or tree_hash(root)!=source:raise ValueError('runtime inputs/source cohort changed')
        inventory=[r['step_id'] for r in manifest['steps']]
        need(inventory,'no actual OS steps collected')
        if len(set(inventory))!=len(inventory) or any(i not in {s for s,_ in plan_steps()} for i in inventory):raise ValueError('unknown/duplicate runtime step')
        for r in manifest['steps']:
            d=safe_path(directory,r['directory'])
            for p,h in r['files'].items():
                f=safe_path(d,p)
                if not f.is_file() or digest(f)!=h:raise ValueError('raw runtime evidence hash mismatch: '+p)
            for w in unique_json(d/'os.json').get('windows',[]):
                cap=w.get('capture',{})
                for image in ([cap] if cap.get('png') else [])+cap.get('preview_crops',[]):
                    png=safe_path(d,image['png']);data=png.read_bytes()
                    if image['png'] not in r['files'] or not data.startswith(b'\x89PNG\r\n\x1a\n') or len(data)<24 or struct.unpack('>II',data[16:24])!=(image['width'],image['height']):raise ValueError('actual OS screenshot missing or PNG dimensions differ')
            actual={p.relative_to(d).as_posix() for p in d.rglob('*') if p.is_file()}
            if actual!=set(r['files']):raise ValueError('runtime raw inventory changed')
            snapshot={name:unique_json(d/(name+'.json')) for name in ('os','database','identity')}
            snapshot.update(action=unique_json(d/'barrier-action.json'),pid=manifest['main_pid'],directory=d)
            if snapshot['os']['kind']!='independent_macos_runtime_observation' or snapshot['os']['observed_at_ms']<manifest['started_at_ms'] or snapshot['os']['observed_at_ms']>r['collected_at_ms']:raise ValueError('stale/non-OS runtime observation')
            steps[r['step_id']]=(r,snapshot,d)
        facts,health=inspect_trace(directory,manifest,nonce,manifest['main_pid'],manifest['started_at_ms'])
    except EvidenceUnavailable as e:errors.append('blocked: '+str(e))
    except (OSError,ValueError,KeyError,TypeError) as e:errors.append(str(e))
    def snap(step):
        need(step in steps,'step not executed: '+step);return steps[step][1]
    def identity():
        from package_identity import inspect_package
        need(steps,'no actual process identity')
        expected=unique_json(directory/'package-inspection.json')
        actual=inspect_package(root,safe_path(root,x['artifact']),source,directory/'independent-package-inspection')
        if actual!=expected or manifest['artifact_sha256']!=actual['archive_sha256']:raise ValueError('actual package differs from run')
        first=None
        for _,s,_ in steps.values():
            p=next((r for r in s['os']['processes'] if r['pid']==s['pid']),None);need(p is not None,'missing OS main process')
            now=(p.get('start_sec'),p.get('start_usec'),p.get('executable'),p.get('device'),p.get('inode'),s['os'].get('boot_sec'),s['os'].get('boot_usec'))
            if first is None:first=now
            if now!=first or any(v is None for v in now) or int(p['start_sec'])*1000+int(p['start_usec'])//1000<manifest['started_at_ms']:raise ValueError('main process/start/boot/image cohort differs')
            guest=p['guest_code']
            if guest.get('validity')!=0 or guest.get('cdhash')!=actual['signature_metadata']['cdhash'] or guest.get('identifier')!='com.lunatranslator.reader':raise ValueError('actual running guest-code signature differs')
            i=s['identity']
            if i['executable_sha256']!=actual['executable_sha256'] or i['seal_sha256']!=actual['sealed_source_identity_sha256'] or i['signature']!=actual['signature_metadata'] or i['profile_id']!=x['profile_id'] or i['source_tree_sha256']!=source:raise ValueError('installed/profile/source identity drift')
        return len(steps)*6
    check('SR6-IDENTITY',identity)
    def discovery():
        for game in x['games']:
            cold=snap(game['id']+'/cold');connected=snap(game['id']+'/connected')
            if any(r['session']['target_exe_path']==game['target_exe_path'] and r['session']['environment_key']==game['environment_key'] for r in cold['database']['sessions']):raise ValueError('game was already hooked before cold discovery')
            s=session_for(connected,game);need(s['sources'] and s['selections'],'no actual sources/explicit body selection')
            need(any(r['code']=='association_ready' for r in s['hook_events']) and any(r['code']=='host_ready' for r in s['hook_events']) and any(r['code']=='process_connected' for r in s['hook_events']),'missing genuine association/ABI handshake/process connection')
            window(cold,'game');window(connected,'main')
        return len(GAMES)*6
    check('SR6-COLD-DISCOVERY',discovery)
    def continuous():
        for game in x['games']:
            seen=[]
            for i in range(1,6):
                s=snap(game['id']+f'/reading-{i}');body=inspect_body(session_for(s,game));need(body,'no actual body')
                b=body[-1];seen.append(b['source_sha256'])
                for role in ('game','main','hud'):visible_text(s,role,b['source_text'])
            need(len(set(seen))==5,'five distinct real sentences not observed: '+game['id'])
        return len(GAMES)*20
    check('SR6-CONTINUOUS-TEXT',continuous)
    def lexical():
        for game in x['games']:
            if game['id']=='sr':continue
            s=snap(game['id']+'/reading-5');session=session_for(s,game);body=inspect_body(session)[-1]
            if not re.search('[\u3040-\u30ff]',body['source_text']):raise ValueError('Chinese sample cannot prove Japanese lexical behavior')
            tokens=inspect_lexical(session,body);hud=window(s,'hud')['capture'];need(hud.get('ocr'),'kana rendering OCR unavailable')
            words=norm(''.join(r['text'] for r in hud['ocr']));need(any(norm(t['reading_kana']) in words for t in tokens if t['reading_kana'] and re.search('[\u4e00-\u9fff]',t['surface'])),'real HUD kana over kanji not visible')
            card=snap(game['id']+'/wordcard-open');need(window(card,'wordcard')['capture'].get('ocr'),'actual dictionary wordcard unavailable')
        return 3*5
    check('SR6-JAPANESE-LEXICAL',lexical)
    def preview():
        need(facts,'no current-run frame ledger')
        for game in x['games']:
            distinct=set();last=0
            for i in range(1,6):
                s=snap(game['id']+f'/reading-{i}');b=inspect_body(session_for(s,game))[-1];preview_text(s,b['source_text'])
                pairs=new_frames(facts,s,last);distinct.update(g['content_hash'] for _,g in pairs);last=s['os']['observed_at_ms']
            need(len(distinct)>=2,'no actual changed image contents: '+game['id'])
        return 4*12
    check('SR6-PREVIEW-CHANGED-FRAMES',preview)
    def isolation():
        a=snap('sakura10th/isolation-before');b=snap('sakura10th/isolation-after')
        before=new_frames(facts,a);after=new_frames(facts,b,a['os']['observed_at_ms'])
        need({f['source_id'] for f,_ in before}.intersection(f['source_id'] for f,_ in after),'producer replaced/stopped during consumer navigation')
        if not {f['capture_id'] for f,_ in before}.intersection(f['capture_id'] for f,_ in after):raise ValueError('consumer change restarted capture')
        session=session_for(b,x['games'][-1]);preview_text(b,inspect_body(session)[-1]['source_text']);return 4
    check('SR6-PRODUCER-ISOLATION',isolation)
    def frozen():
        for game in x['games']:
            if game['id']=='sr':continue
            a=snap(game['id']+'/wordcard-open');b=snap(game['id']+'/wordcard-retained');wid=window(a,'wordcard')['window_id']
            if window(b,'wordcard')['window_id']!=wid:raise ValueError('frozen wordcard window replaced after new sentence')
            old=[r['fact'] for r in facts if r['fact']['kind']=='word_card_snapshot' and r['observed_at_ms']<=a['os']['observed_at_ms']];need(old,'genuine frozen card identity unavailable');card=old[-1]
            later=[r['fact'] for r in facts if r['fact']['kind']=='word_card_snapshot' and a['os']['observed_at_ms']<r['observed_at_ms']<=b['os']['observed_at_ms']]
            if any(r!=card for r in later):raise ValueError('card fixed unit/revision/token changed with new dialogue')
            session=session_for(b,game);body=inspect_body(session);new=body[-1]
            originals=[r for r in body if r['unit_id']==card['unit_id'] and r['text_revision']==card['text_revision']]
            need(len(originals)==1,'frozen source revision unavailable')
            if originals[0]['source_sha256']!=card['source_hash']:raise ValueError('frozen card source hash differs')
            geometry=read_jsonl(b['directory']/'geometry-lifecycle.jsonl')
            need(any(r.get('kind')=='geometry_fact' and r['fact'].get('phase')=='fit_completed' and r['fact'].get('reason')=='confirmed_content_fit' and a['os']['observed_at_ms']<r['observed_at_ms']<=b['os']['observed_at_ms'] for r in geometry),'no actual confirmed content fit while old card retained')
            if any(r.get('kind')=='geometry_fact' and r['fact'].get('card_id')==card['card_id'] and r['fact'].get('phase') in ('card_close_requested','native_card_destroyed') and a['os']['observed_at_ms']<r['observed_at_ms']<=b['os']['observed_at_ms'] for r in geometry):raise ValueError('old card closed during content fit')
            if new['unit_id']==card['unit_id'] and new['text_revision']==card['text_revision']:raise ValueError('no new source while frozen card retained')
            tokens=[r for r in session['tokens'] if r['token_index']==card['token_index'] and any(l['lexical_id']==r['lexical_id'] and l['unit_id']==card['unit_id'] and l['text_revision']==card['text_revision'] for l in session['lexical'])];need(tokens,'frozen dictionary token unavailable')
            visible_text(b,'wordcard',tokens[0]['lemma'])
        # Actual native destruction must follow each normal user closure action;
        # window disappearance or a user completed barrier alone is insufficient.
        for action in ('user-geometry','escape','outside-pointer','owner-scroll'):
            a=snap('sakura10th/card-'+action+'-open');b=snap('sakura10th/card-'+action+'-closed');wid=window(a,'wordcard')['window_id']
            if any(w['window_id']==wid for w in b['os']['windows']):raise ValueError('wordcard native surface did not close for '+action)
            relevant=[r['fact'] for r in facts if r['fact']['kind']=='word_card_snapshot' and r['observed_at_ms']<=a['os']['observed_at_ms']];need(relevant,'missing current card metadata');card=relevant[-1]
            trace=read_jsonl(b['directory']/'geometry-lifecycle.jsonl')
            need(any(r.get('kind')=='geometry_fact' and r.get('fact',{}).get('card_id')==card['card_id'] and r['fact'].get('phase')=='native_card_destroyed' and a['os']['observed_at_ms']<=r['observed_at_ms']<=b['os']['observed_at_ms'] for r in trace),'no exact native card destruction fact for '+action)
        return 3*5+4*2
    check('SR6-FROZEN-WORDCARD',frozen)
    def drain():
        for game in x['games']:
            before=snap(game['id']+'/reading-5');after=snap(game['id']+'/exited');session=session_for(after,game);s=session['session'];sid=s['session_id']
            if s['state']!='stopped' or s['ended_at_ms'] is None or any(o['state']!='stopped' for o in session['operations']):raise ValueError('actual application durable stopped barrier missing')
            f=unique_json(after['directory']/'host'/sid/'final-snapshot.json')
            if f['session_id']!=sid or f['host_running'] or f['exit_code']!=0 or not f['shutdown_acknowledged'] or f['integrity_lost'] or f['errors']:raise ValueError('Host final EOF/ACK/integrity incomplete')
            own=unique_json(before['directory']/'owned-processes.json');host=[r['pid'] for r in own if 'lunahook-host.exe' in r['command']];need(host,'actual owned Host PID observation unavailable')
            own_after=unique_json(after['directory']/'owned-processes.json')
            if any(r['pid'] in host for r in own_after):raise ValueError('old owned Host still exists after natural exit')
            lock=s['target_identity_sha256']+'.lock';old=[r for r in lsof_entries((before['directory']/'lsof.txt').read_text()) if r.get('name','').endswith('/'+lock)]
            need(old and all(r.get('inode') for r in old),'actual expected-target lock FD/inode unavailable')
            now=lsof_entries((after['directory']/'lsof.txt').read_text())
            if any(r.get('inode') in {o['inode'] for o in old} for r in now):raise ValueError('old target lock FD/inode not released before next game')
            text_rows=[json.loads(r) for r in (after['directory']/'host'/sid/'stdout.jsonl').read_text().splitlines()]
            inspect_host_tail(session,text_rows,f)
            if any(r['integrity_state']!='intact' for r in session['raw']):raise ValueError('application journal contains integrity gap')
        return 4*8
    check('SR6-NATURAL-EXIT-SWITCH',drain)
    def progress():
        for game in x['games']:
            a=snap(game['id']+'/connected');b=snap(game['id']+'/reading-5');c=session_for(a,game);d=session_for(b,game)
            ra=a['database']['profile_meta'][0]['commit_revision'];rb=b['database']['profile_meta'][0]['commit_revision']
            if rb<=ra or len(d['raw'])<=len(c['raw']) or len(d['body'])<=len(c['body']):raise ValueError('no durable writer/relay progress under reading consumers')
            if any(r['code'] in ('queue_full','storage_unconfirmed') or r['integrity']!='intact' for r in d['hook_events']):raise ValueError('writer/relay failure or integrity loss reported')
        return 4*4
    check('SR6-QUEUE-PROGRESS',progress)
    def sleep():
        a=snap('sakura10th/sleep-before');b=snap('sakura10th/wake-after');start=a['os']['observed_at_ms'];end=b['os']['observed_at_ms']
        need(power_sleep_wake((b['directory']/'power.txt').read_text(),start,end),'no actual in-run Sleep then user Wake; display sleep/lock/DarkWake excluded')
        before=new_frames(facts,a);after=new_frames(facts,b,start)
        if not {f['source_id'] for f,_ in before}.intersection(f['source_id'] for f,_ in after):raise ValueError('sleep recovery replaced producer')
        need(any(f['captured_at_ms']>start for f,_ in after),'no actual post-wake captured frame')
        sa=session_for(a,x['games'][-1]);sb=session_for(b,x['games'][-1])
        if sa['session']['session_id']!=sb['session']['session_id'] or inspect_body(sa)[-1]['source_sha256']==inspect_body(sb)[-1]['source_sha256']:raise ValueError('sleep recovery lacks same session and new real sentence')
        preview_text(b,inspect_body(sb)[-1]['source_text']);return 5
    check('SR6-REAL-SLEEP-WAKE',sleep)
    required=[s for s,_ in plan_steps()]
    missing=sorted(set(required)-set(steps))
    if missing:errors.append('required runtime steps not executed: '+', '.join(missing))
    if not manifest.get('completed'):errors.append('runtime collector did not complete all action/observation steps')
    errors.extend(f"{r['id']} {r['status']}: {'; '.join(r['errors'])}" for r in cases.values() if r['status']!='passed')
    if errors:
        # Global source/nonce/trace failure invalidates otherwise locally valid
        # case results. Never promote a subset under a broken run identity.
        status='blocked' if all(e.startswith(('blocked:','required runtime steps','runtime collector')) or ' blocked:' in e for e in errors) else 'failed'
    else:status='passed'
    return {'schema_version':1,'kind':'independently_recomputed_packaged_stability','status':status,'errors':errors,'execution_nonce':nonce,'source_tree_sha256':source,'artifact_sha256':manifest.get('artifact_sha256'),'main_pid':manifest.get('main_pid'),'cases':list(cases.values()),'assertion_count':sum(r['assertion_count'] for r in cases.values()),'fixture_substitution':False,'product_packaged_native_game_tests_executed':bool(steps) and cases['SR6-IDENTITY']['status']=='passed','scope':'SR6 stability only; not first-product 153-combination release acceptance','executed_at_ms':int(datetime.now().timestamp()*1000)}
