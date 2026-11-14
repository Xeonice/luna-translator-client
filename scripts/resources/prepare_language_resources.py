#!/usr/bin/env python3
"""Build and stage the fixed offline MeCab/UniDic and licensed dictionary index.

No network request is performed. Input snapshots must be acquired and reviewed
before invoking this developer packaging command; the product never downloads them.
"""
from __future__ import annotations
import argparse, hashlib, json, shutil, sqlite3, time
from pathlib import Path
from urllib.parse import quote

ROOT=Path(__file__).resolve().parents[2]
LANG=ROOT/'resources/language'
DOWNLOADS=ROOT/'.build/language-downloads'
VERSION='wiktionary-zh-ja-20261018+jmdict-20261012-v1'
NORMALIZATION='kana-hiragana-exact-v1'


def sha(path):
    h=hashlib.sha256()
    with Path(path).open('rb') as stream:
        for block in iter(lambda:stream.read(1024*1024),b''):h.update(block)
    return h.hexdigest()


def canonical(value):return json.dumps(value,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode()
def normalize(text):return ''.join(chr(ord(c)-0x60) if '\u30a1'<=c<='\u30f6' else c for c in text.strip())


def source_manifest():
    sources=[]
    for variant in ['simplified','traditional']:
        path=DOWNLOADS/f'zh-ja-{variant}.jsonl'
        meta=json.loads(path.with_suffix('.jsonl.source.json').read_text())
        if sha(path)!=meta['sha256']:raise ValueError(f'dictionary source hash changed: {path}')
        sources.append(dict(source_id=f'zhwiktionary-{variant}',display_name=f'中文维基词典（{"简体" if variant=="simplified" else "繁体"}导出）',snapshot_version='2026-10-18;dump-2026-10-01',source_sha256=meta['sha256'],source_url=meta['url'],license_ref='licenses/wiktionary/CC-BY-SA-4.0.txt',attribution_text='Copyright Chinese Wiktionary contributors; extraction Kaikki.org / Wiktextract. CC BY-SA 4.0. Offline storage conversion; original Chinese glosses retained.'))
    old=ROOT/'apps/desktop/src-tauri/resources/language'
    meta=json.loads((old/'manifest.json').read_text())
    # Once staging replaces the old resource folder, reuse the retained source
    # snapshot and its exact reviewed provenance; never learn a new accepted hash.
    archive=DOWNLOADS/'jmdict.jsonl'
    archived_meta=DOWNLOADS/'jmdict.source.json'
    if not archive.exists():
        if meta.get('dictionary')!='JMdict':raise ValueError('missing retained JMdict source snapshot')
        shutil.copy2(old/'jmdict.jsonl',archive);archived_meta.write_bytes(canonical(meta))
    meta=json.loads(archived_meta.read_text())
    if sha(archive)!=meta['output_sha256']:raise ValueError('JMdict retained source hash differs')
    sources.append(dict(source_id='jmdict-en',display_name='JMdict / EDRDG（英语补充）',snapshot_version='2026-10-12;3.6.2',source_sha256=meta['output_sha256'],source_url=meta['source_url'],license_ref='licenses/jmdict/CC-BY-SA-4.0.txt',attribution_text=meta['attribution']))
    return sources


def build_dictionary(rebuild=False):
    sources=source_manifest()
    schema=ROOT/'crates/app-infra/src/storage/migrations/001_dictionary.sql'
    provenance={'schema_version':1,'resource_version':VERSION,'index_schema_version':1,'normalization_version':NORMALIZATION,'production_schema_sha256':sha(schema),'sources':sources}
    provenance_path=LANG/'dictionary-provenance.json';provenance_bytes=canonical(provenance)+b'\n';provenance_hash=hashlib.sha256(provenance_bytes).hexdigest()
    database=LANG/'dictionary.sqlite3'
    if not rebuild and database.is_file() and provenance_path.is_file() and provenance_path.read_bytes()==provenance_bytes:
        with sqlite3.connect(f'file:{database}?mode=ro',uri=True) as db:
            if db.execute('SELECT manifest_sha256 FROM dictionary_meta').fetchone()==(provenance_hash,):return provenance
    temporary=database.with_suffix('.building.sqlite3')
    if temporary.exists():temporary.unlink()
    db=sqlite3.connect(temporary)
    db.executescript(schema.read_text())
    db.execute('BEGIN')
    db.execute('INSERT INTO dictionary_meta VALUES (1,?,?,?,?,?)',(VERSION,provenance_hash,1,NORMALIZATION,int(time.time()*1000)))
    for source in sources:
        db.execute('INSERT INTO dictionary_sources VALUES (?,?,?,?,?,?,?)',tuple(source[k] for k in ['source_id','display_name','snapshot_version','source_sha256','source_url','license_ref','attribution_text']))
    entries=0;senses_count=0
    def insert(source_id,key,lemma,reading,pos,kind,forms,senses):
        nonlocal entries,senses_count
        if not lemma or not senses:return
        entry_id=hashlib.sha256((source_id+'\0'+key).encode()).hexdigest()
        db.execute('INSERT INTO dictionary_entries VALUES (?,?,?,?,?,?,?)',(entry_id,source_id,key,lemma,reading,json.dumps(pos,ensure_ascii=False),kind))
        seen=set()
        for text,form_kind,rank in [(lemma,'lemma',0)]+[(f,'surface',1) for f in forms]+([(reading,'reading',2)] if reading else []):
            normalized=normalize(text)
            if not normalized or (normalized,form_kind) in seen:continue
            seen.add((normalized,form_kind));form_id=hashlib.sha256((entry_id+'\0'+normalized+'\0'+form_kind).encode()).hexdigest()
            db.execute('INSERT INTO dictionary_forms VALUES (?,?,?,?,?,?)',(form_id,entry_id,text,normalized,form_kind,rank))
        for index,(language,gloss,notes,url) in enumerate(senses):
            db.execute('INSERT INTO dictionary_senses VALUES (?,?,?,?,?,?)',(entry_id,index,language,gloss,notes,url));senses_count+=1
        entries+=1
    for variant in ['simplified','traditional']:
        path=DOWNLOADS/f'zh-ja-{variant}.jsonl';source_id=f'zhwiktionary-{variant}'
        for lineno,line in enumerate(path.open(),1):
            entry=json.loads(line)
            if entry.get('lang_code')!='ja':continue
            lemma=entry.get('word','');forms=[f.get('form','') for f in entry.get('forms',[]) if f.get('form')];reading=None
            for form in entry.get('forms',[]):
                ruby=form.get('ruby',[])
                if ruby and form.get('form')==lemma:
                    reading=''.join(pair[1] for pair in ruby if len(pair)==2) or None;break
            url='https://zh.wiktionary.org/wiki/'+quote(lemma,safe='')+'#日語';senses=[]
            ids=[]
            for sense in entry.get('senses',[]):
                if sense.get('id'):ids.append(sense['id'])
                notes=json.dumps({'tags':sense.get('tags',[]),'source_sense_id':sense.get('id')},ensure_ascii=False)
                for gloss in sense.get('glosses',[]):
                    if isinstance(gloss,str) and gloss.strip():senses.append(('zh',gloss,notes,url))
            insert(source_id,f'{lineno}:'+','.join(ids),lemma,reading,[entry.get('pos_title') or entry.get('pos','unknown')],'zh_base',forms,senses)
    for line in (DOWNLOADS/'jmdict.jsonl').open():
        entry=json.loads(line);headwords=entry.get('headwords',[]);readings=entry.get('readings',[])
        lemma=next(iter(headwords or readings),'');url='https://www.edrdg.org/jmdict/edict_doc.html#'+quote(entry['id'],safe='')
        senses=[('en',gloss,None,url) for gloss in entry.get('glosses',[]) if isinstance(gloss,str) and gloss.strip()]
        insert('jmdict-en',entry['id'],lemma,next(iter(readings),None),[],'en_supplement',headwords+readings,senses)
    db.execute('PRAGMA user_version=1');db.commit()
    if db.execute('PRAGMA foreign_key_check').fetchall():raise ValueError('dictionary foreign key errors')
    if db.execute('PRAGMA integrity_check').fetchone()!=('ok',):raise ValueError('dictionary integrity check failed')
    db.execute('VACUUM');db.close();temporary.replace(database);provenance_path.write_bytes(provenance_bytes)
    print(json.dumps({'dictionary_entries':entries,'dictionary_senses':senses_count},ensure_ascii=False))
    return provenance


def prepare(resources:Path|None=None,rebuild=False):
    provenance=build_dictionary(rebuild)
    files=[]
    for base in [LANG,ROOT/'resources/licenses']:
        for path in sorted(base.rglob('*')):
            if not path.is_file() or path.name=='manifest.json':continue
            if path.is_symlink():raise ValueError(f'resource cannot be a symbolic link: {path}')
            files.append({'path':str(path.relative_to(ROOT/'resources')),'size':path.stat().st_size,'sha256':sha(path)})
    manifest={'schema_version':1,'resource_version':'mecab-0.996_unidic-2025.12_'+VERSION,'mecab_version':'0.996','mecab_source_commit':'61b90ba6e669dc2d7d533d4a80d206f3b31d52b1','unidic_version':'2025.12','dictionary_version':VERSION,'dictionary_provenance_sha256':sha(LANG/'dictionary-provenance.json'),'normalization_version':NORMALIZATION,'files':files,'offline_first_launch':True}
    manifest['sources']=[dict(json.loads((DOWNLOADS/name).read_text()),kind=kind,license_ref=license_ref)
                         for name,kind,license_ref in [('mecab-source.tar.gz.source.json','mecab','licenses/mecab/BSD'),('unidic-cwj-202512.zip.source.json','unidic','licenses/unidic/BSD')]]
    manifest['supported_platform']='macos-arm64'
    with sqlite3.connect(f'file:{LANG / "dictionary.sqlite3"}?mode=ro',uri=True) as database:
        manifest['dictionary_counts']={'entries':database.execute('SELECT count(*) FROM dictionary_entries').fetchone()[0],'senses':database.execute('SELECT count(*) FROM dictionary_senses').fetchone()[0],'by_content_kind':dict(database.execute('SELECT content_kind,count(*) FROM dictionary_entries GROUP BY content_kind'))}
    (LANG/'manifest.json').write_bytes(canonical(manifest)+b'\n')
    if resources:
        resources.mkdir(parents=True,exist_ok=True)
        destination=resources/'language'
        if destination.is_symlink():raise ValueError('language staging directory cannot be a symbolic link')
        if destination.exists():shutil.rmtree(destination)
        shutil.copytree(LANG,destination)
        for name in ['mecab','unidic','wiktionary','jmdict']:shutil.copytree(ROOT/'resources/licenses'/name,resources/'licenses'/name,dirs_exist_ok=True)
        worker=ROOT/'target/release/language-worker'
        if not worker.is_file():raise FileNotFoundError('build the native language-worker --release --locked before staging')
        shutil.copy2(worker,resources/'language-worker')
    print(json.dumps({'version':manifest['resource_version'],'manifest_sha256':sha(LANG/'manifest.json'),'dictionary_provenance_sha256':manifest['dictionary_provenance_sha256'],'files':len(files)},ensure_ascii=False))
    return manifest


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--resources',type=Path);parser.add_argument('--rebuild-dictionary',action='store_true');args=parser.parse_args();prepare(args.resources,args.rebuild_dictionary)
