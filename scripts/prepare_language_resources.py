#!/usr/bin/env python3
"""Refresh redistributable offline Japanese lexicon; never runs in the product.
Data remains CC BY-SA 4.0, attributed to EDRDG and jmdict-simplified.
Pinned source URL/hash recorded for reproducible audit. Download is build-time only.
"""
import argparse,hashlib,io,json,tarfile,urllib.request
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'apps/desktop/src-tauri/resources/language'
DEFAULT='https://github.com/scriptin/jmdict-simplified/releases/download/3.6.2%2B20261012204142/jmdict-eng-3.6.2%2B20261012204142.json.tgz'
def main():
 p=argparse.ArgumentParser();p.add_argument('--url',default=DEFAULT);a=p.parse_args()
 raw=urllib.request.urlopen(a.url).read()
 with tarfile.open(fileobj=io.BytesIO(raw),mode='r:gz') as t:
  item=next(x for x in t.getmembers() if x.name.endswith('.json'))
  data=json.load(t.extractfile(item))
 OUT.mkdir(parents=True,exist_ok=True)
 lines=[]
 for word in data['words']:
  glosses=[]
  for sense in word['sense']:
   for g in sense['gloss']:
    if g.get('lang','eng')=='eng' and g['text'] not in glosses:glosses.append(g['text'])
  if glosses:
   entry={'id':word['id'],'headwords':[x['text'] for x in word.get('kanji',[])], 'readings':[x['text'] for x in word.get('kana',[])], 'glosses':glosses,'source':'JMdict / EDRDG','language':'en'}
   lines.append(json.dumps(entry,ensure_ascii=False,separators=(',',':')))
 text='\n'.join(lines)+'\n'
 (OUT/'jmdict.jsonl').write_text(text)
 manifest={'schema_version':1,'dictionary':'JMdict','version':data.get('version'),'dictionary_date':data.get('dictDate'),'entries':len(lines),'language':'en','source_url':a.url,'source_sha256':hashlib.sha256(raw).hexdigest(),'output_sha256':hashlib.sha256(text.encode()).hexdigest(),'license':'CC-BY-SA-4.0','attribution':'Copyright James William Breen and the Electronic Dictionary Research and Development Group. JSON conversion scriptin/jmdict-simplified. Flattened/indexed for Luna Reader. Glosses unchanged.','license_url':'https://www.edrdg.org/edrdg/licence.html','project_url':'https://github.com/scriptin/jmdict-simplified','refresh':'Run scripts/prepare_language_resources.py with a newer official release URL, review manifest and ship through an application update.'}
 (OUT/'manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n')
 for name,url in [('EDRDG-LICENSE.html','https://www.edrdg.org/edrdg/licence.html'),('CC-BY-SA-4.0.txt','https://creativecommons.org/licenses/by-sa/4.0/legalcode.txt'),('jmdict-simplified-README.md','https://raw.githubusercontent.com/scriptin/jmdict-simplified/master/README.md')]:
  (OUT/name).write_bytes(urllib.request.urlopen(urllib.request.Request(url,headers={'User-Agent':'LunaReader-resource-builder/1'})).read())
 print(json.dumps(manifest,ensure_ascii=False,indent=2))
if __name__=='__main__':main()
