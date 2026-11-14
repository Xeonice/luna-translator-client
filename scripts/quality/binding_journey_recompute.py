"""Independent retained-DOM and compiled-producer replay for binding diagnostics.

No browser report status is used as an oracle. This replay does not operate a
Native window or turn a controlled component branch into game acceptance.
"""
from __future__ import annotations

from html.parser import HTMLParser
from datetime import datetime, timezone
import json
from pathlib import Path
import subprocess
import tempfile

from binding_phase_guard import checked_ref, load, ref
from binding_journey import (COMPONENT_CASES, DIAGNOSTIC_METHODS, DIAGNOSTIC_SCENES,
                             observation_verdict, scene_verdict)


class Element:
    def __init__(self, tag, attrs=(), parent=None):
        self.tag, self.attrs, self.parent = tag, dict(attrs), parent
        self.children = []

    def text(self, include_hidden=False):
        if (self.tag in {'script', 'style', 'svg'} or not include_hidden and ('hidden' in self.attrs
                or self.attrs.get('aria-hidden') == 'true'
                or 'display:none' in self.attrs.get('style', '').replace(' ', ''))):
            return ''
        return ' '.join(x.text(include_hidden) if isinstance(x, Element) else x for x in self.children)

    def descendants(self):
        for child in self.children:
            if isinstance(child, Element):
                yield child
                yield from child.descendants()

    def label(self):
        return self.attrs.get('aria-label') or self.attrs.get('title') or self.text().strip()


class Document(HTMLParser):
    VOID = {'input', 'img', 'br', 'hr', 'meta', 'link', 'source', 'area', 'wbr', 'embed', 'col', 'base', 'param', 'track'}

    def __init__(self, html):
        super().__init__(convert_charrefs=True)
        self.root = Element('root')
        self.stack = [self.root]
        self.feed(html)
        self.close()

    def handle_starttag(self, tag, attrs):
        node = Element(tag, attrs, self.stack[-1])
        self.stack[-1].children.append(node)
        if tag not in self.VOID:
            self.stack.append(node)

    def handle_startendtag(self, tag, attrs):
        self.handle_starttag(tag, attrs)
        if tag not in self.VOID:
            self.handle_endtag(tag)

    def handle_endtag(self, tag):
        for index in range(len(self.stack) - 1, 0, -1):
            if self.stack[index].tag == tag:
                self.stack = self.stack[:index]
                break

    def handle_data(self, data):
        self.stack[-1].children.append(data)


def normalized(value):
    return ''.join(value.split())


def dom_errors(identity, observations, html, events):
    """Check the claimed observations against independently parsed actual DOM."""
    doc = Document(html)
    nodes = list(doc.root.descendants())
    text = doc.root.text()
    norm = normalized(text)
    buttons = [x for x in nodes if x.tag == 'button' or x.attrs.get('role') == 'button']
    labels = [x.label() for x in buttons]
    errors = []

    def require(condition, message):
        if not condition:
            errors.append(identity + ': ' + message)

    def contains(value):
        return normalized(value) in norm

    if 'visibleText' in observations:
        # The runner retains textContent, which includes collapsed Select option
        # text. Match those original bytes separately from visible-control rules.
        require(normalized(observations['visibleText']) in normalized(doc.root.text(True)),
                'observed textContent differs from retained DOM')
    if identity == COMPONENT_CASES[0]:
        actual = observations['selectionButton']
        present = any('选择对应的游戏窗口' in x for x in labels)
        require(actual['visible'] == present, 'selection control visibility differs from DOM')
        if actual.get('label'):
            require(actual['label'] in labels, 'selection label differs from DOM')
        require(contains(observations['timeText']), 'first-frame time differs from DOM')
        # This case is a no-frame production DTO. Reject a fabricated frame even
        # if a dishonest report uses imageCount=0.
        preview_images = [x for x in nodes if x.tag == 'img' and x.attrs.get('src', '').startswith('data:image/')]
        require(len(preview_images) == observations['imageCount'], 'first-frame image count differs from DOM')
    elif identity == COMPONENT_CASES[1]:
        present = any('切换并接入' in x for x in labels)
        require(present == observations['sameTargetSwitchVisible'], 'same-target switch differs from DOM')
    elif identity == COMPONENT_CASES[2]:
        items = [node for node in nodes if node.tag == 'li' and node.parent
                 and node.parent.attrs.get('aria-label') == '接入流程']
        actual = [normalized(''.join(child if isinstance(child, str) else child.text()
                                    for child in node.children if isinstance(child, str) or child.tag != 'b'))
                  for node in items]
        require(actual == observations['stageLabels'], 'stage observations differ from retained DOM')
    elif identity == COMPONENT_CASES[3]:
        checkboxes = [x for x in nodes if x.tag == 'input' and x.attrs.get('type') == 'checkbox'
                      or x.attrs.get('role') == 'checkbox']
        source_boxes = [x for x in checkboxes if not ('记住' in (x.parent.text() if x.parent else x.label()))]
        require(len(source_boxes) >= observations['sourceCheckboxCount'], 'source checkbox count exceeds actual DOM')
        require(any('开始阅读' in x for x in labels) == observations['beginReadingButton']['visible'],
                'begin-reading control differs from DOM')
        require(any(x.attrs.get('role') == 'dialog' for x in nodes) == observations['dialogStillOpen'],
                'draft dialog visibility differs from DOM')
        before, after = observations.get('beforeFacts'), observations.get('afterFacts')
        require(isinstance(before, dict) and isinstance(after, dict), 'actual draft before/after facts missing')
        if isinstance(before, dict) and isinstance(after, dict):
            def commits(facts):
                return (len(facts['commands'])
                        + sum(row.get('method') in {'beginReading', 'selectSources', 'selectSource'}
                              for row in facts['transportCalls']))
            require(commits(after) - commits(before) == observations['draftClickCommandCount'],
                    'unreported begin-reading transport commit')
            for field, facts_key in [('draftWindowEffectCount', 'windows'), ('draftAssociationCount', 'associations')]:
                actual = len(after[facts_key]) - len(before[facts_key])
                if field == 'draftAssociationCount':
                    actual += (sum(x.get('method') in {'associate', 'associateGameWindow'} for x in after['transportCalls'])
                               - sum(x.get('method') in {'associate', 'associateGameWindow'} for x in before['transportCalls']))
                require(actual == observations[field],
                        'draft effects differ from before/after trace')
            require(after['events'] == events, 'draft final events differ from retained trace')
    elif identity == COMPONENT_CASES[4]:
        require(contains(observations['sampleText']) == observations['longSampleRetained'],
                'full source sample differs from DOM')
        if observations['productionDetailFixtureAvailable']:
            # Expected values have already been bound to the actual producer
            # response by the inspector. Compare each observed source field.
            for row in observations['sourceDetails']:
                source_nodes = [x for x in nodes if x.attrs.get('data-source-id') == row['source_id']]
                require(bool(source_nodes), 'missing per-source detail DOM identity')
                source_text = normalized(' '.join(x.text() for x in source_nodes))
                for field in row['fields']:
                    require(normalized(field['label']) in source_text and normalized(field['actual']) in source_text,
                            'source field differs from its retained DOM')
    elif identity == COMPONENT_CASES[5]:
        remember = [x for x in nodes if x.tag == 'label' and '记住' in x.text()
                    or '记住' in x.attrs.get('aria-label', '')]
        boxes = [c for x in remember for c in [x, *x.descendants()]
                 if c.tag == 'input' and c.attrs.get('type') == 'checkbox' or c.attrs.get('role') == 'checkbox']
        require(bool(boxes) == observations['rememberCheckbox']['visible'], 'remember checkbox differs from DOM')
        if boxes:
            require(('disabled' not in boxes[0].attrs and boxes[0].attrs.get('aria-disabled') != 'true')
                    == observations['rememberCheckbox']['enabled'], 'remember availability differs from DOM')
    else:
        # Count the real instrumented port entry in both transport modes. The
        # old controlled DesignPort marker is absent from production RPC and
        # cannot itself prove that an actual picker method was invoked.
        explicit = [x for x in events if x.get('kind') == 'port_call'
                    and x.get('method') == 'selectWindowPreview']
        require(len(explicit) == observations['pickerCallsAfterIntent'], 'picker count differs from raw trace')
        before = observations.get('beforeFacts')
        require(isinstance(before, dict) and isinstance(before.get('events'), list),
                'actual pre-intent association facts missing')
        if isinstance(before, dict) and isinstance(before.get('events'), list):
            actual = sum(x.get('kind') == 'port_call' and x.get('method') == 'associate'
                         for x in before['events'])
            require(type(observations['associationCallsBeforeIntent']) is int
                    and actual == observations['associationCallsBeforeIntent'],
                    'association count differs from actual pre-intent port calls')
    return errors


LAYOUT_REPLAY = r'''
import fs from 'node:fs/promises';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
const [root, inputFile] = process.argv.slice(1);
const entries = JSON.parse(await fs.readFile(inputFile,'utf8'));
const {launchLockedBrowser} = await import(pathToFileURL(path.join(root,'apps/desktop/tests/visual/locked-browser.mjs')));
const {browser} = await launchLockedBrowser();
const result=[];
try {
 for (const entry of entries) {
  const g=entry.dom.geometry;
  const context=await browser.newContext({viewport:{width:g.width,height:g.height},deviceScaleFactor:g.devicePixelRatio??1,locale:g.locale??'ja'});
  const page=await context.newPage();
  await page.route('**/*',route=>['data:','blob:','about:'].includes(new URL(route.request().url()).protocol)?route.continue():route.abort());
  let html=await fs.readFile(entry.html,'utf8');
  html=html.replace(/<script\b[^>]*>[\s\S]*?<\/script\s*>/gi,'');
  html=html.replace(/<link\b[^>]*>/gi,tag=>{const match=tag.match(/href="([^"]+)"/);return match&&match[1].includes('.css')?'':tag});
  html=html.replace('</head>','<style>'+entry.css.replaceAll('</style','<\\/style')+'</style></head>');
  await page.setContent(html,{waitUntil:'load'});
  await page.evaluate(async()=>{await document.fonts.ready;for(const node of document.querySelectorAll('[data-binding-observed-scroll-top],[data-binding-observed-scroll-left]')){node.scrollTop=Number(node.dataset.bindingObservedScrollTop??0);node.scrollLeft=Number(node.dataset.bindingObservedScrollLeft??0)}await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))});
  const actual=await page.evaluate(()=>{
   const visible=node=>!!node&&!node.closest('[hidden]')&&getComputedStyle(node).display!=='none'&&getComputedStyle(node).visibility!=='hidden'&&node.getBoundingClientRect().width>0&&node.getBoundingClientRect().height>0;
   const label=node=>node.getAttribute('aria-label')||[...(node.labels??[])].map(item=>item.innerText).join(' ')||node.innerText||node.getAttribute('title')||'';
   const rect=node=>node.getBoundingClientRect().toJSON();
   const buttons=[...document.querySelectorAll('button,[role="button"]')].filter(visible).map(node=>({label:label(node).trim(),disabled:node.matches(':disabled')||node.getAttribute('aria-disabled')==='true',pressed:node.getAttribute('aria-pressed'),rect:rect(node)}));
   const checkboxes=[...document.querySelectorAll('input[type="checkbox"],[role="checkbox"]')].filter(visible).map(node=>({label:label(node).trim(),checked:node.checked??node.getAttribute('aria-checked')==='true',disabled:node.matches(':disabled')||node.getAttribute('aria-disabled')==='true',sourceId:node.getAttribute('data-source-id')??node.closest('[data-source-id]')?.getAttribute('data-source-id')??(node.value!=='on'?node.value:null)}));
   const sourceRows=[...document.querySelectorAll('[role="dialog"]')].filter(visible).flatMap(dialog=>[...dialog.querySelectorAll('[data-source-id]')].map(node=>{const controls=[...node.querySelectorAll('input[type="checkbox"],[role="checkbox"]')];return{sourceId:node.dataset.sourceId,checkboxCount:controls.length,visibleCheckboxCount:controls.filter(visible).length,enabledCheckboxCount:controls.filter(control=>visible(control)&&!control.matches(':disabled')&&control.getAttribute('aria-disabled')!=='true').length}}));
   const previews=[...document.querySelectorAll('[aria-label="运行窗口预览"]')].filter(visible).map(node=>({state:node.getAttribute('data-preview-state'),text:node.innerText,rect:rect(node),time:node.querySelector('time')?.textContent??null,dateTime:node.querySelector('time')?.getAttribute('datetime')??null,images:[...node.querySelectorAll('img')].map(image=>({alt:image.alt,naturalWidth:image.naturalWidth,naturalHeight:image.naturalHeight,complete:image.complete}))}));
   const fonts=[...document.querySelectorAll('h1,h2,button,input,[aria-label="运行窗口预览"]')].filter(visible).slice(0,80).map(node=>{const style=getComputedStyle(node);return{tag:node.tagName,label:label(node).trim().slice(0,120),family:style.fontFamily,size:style.fontSize,weight:style.fontWeight,lineHeight:style.lineHeight,letterSpacing:style.letterSpacing}});
   const dialogs=[...document.querySelectorAll('[role="dialog"]')].filter(visible).map(node=>({text:node.innerText,rect:rect(node),ariaLabel:node.getAttribute('aria-label')}));
   const details=[...document.querySelectorAll('details')].map(node=>({text:node.textContent,open:node.open,visible:visible(node),rect:rect(node)}));
   const scrollOwners=[...document.querySelectorAll('*')].filter(node=>visible(node)&&['auto','scroll'].includes(getComputedStyle(node).overflowY)&&node.scrollHeight>node.clientHeight+1).map(node=>({tag:node.tagName,role:node.getAttribute('role'),key:node.getAttribute('data-ui-scroll'),rect:rect(node),scrollTop:node.scrollTop,scrollHeight:node.scrollHeight,clientHeight:node.clientHeight}));
   const sourceDefinitions=[...document.querySelectorAll('[role="dialog"] [data-source-id]')].map(node=>({sourceId:node.dataset.sourceId,fields:[...node.querySelectorAll('dt')].filter(term=>visible(term)&&term.nextElementSibling?.tagName==='DD'&&visible(term.nextElementSibling)).map(term=>({label:term.textContent.trim(),value:term.nextElementSibling.textContent.trim()}))}));
   const progressStages=[...document.querySelectorAll('[aria-label="自动接入阶段"] li')].filter(visible).map(node=>({text:node.innerText.trim(),current:node.getAttribute('aria-current')==='step'||/(?:^|[_ -])current(?:[_ -]|$)/.test(node.className),complete:/(?:^|[_ -])complete(?:[_ -]|$)/.test(node.className),className:node.className,ariaCurrent:node.getAttribute('aria-current')}));
   return {text:document.body.innerText,progressStages,sourceRows,headings:[...document.querySelectorAll('h1,h2,h3,[role="heading"]')].filter(visible).map(node=>node.innerText.trim()),dialogs,details,scrollOwners,sourceDefinitions,buttons,checkboxes,previews,fonts:{ready:document.fonts.status,samples:fonts},geometry:{width:innerWidth,height:innerHeight,devicePixelRatio,locale:navigator.language,theme:document.documentElement.dataset.theme,zoom:getComputedStyle(document.documentElement).zoom,bodyScrollTop:document.body.scrollTop,rootScrollTop:document.documentElement.scrollTop,rootScrollWidth:document.documentElement.scrollWidth,rootClientWidth:document.documentElement.clientWidth}};
  });
  result.push({id:entry.id,actual}); await context.close();
 }
}finally{await browser.close()}
process.stdout.write(JSON.stringify(result));
'''


def close_values(expected, actual, path='root'):
    if type(expected) is dict and type(actual) is dict:
        if set(expected) != set(actual):
            return [path + ': field set differs']
        return [error for key in expected for error in close_values(expected[key], actual[key], path + '.' + key)]
    if type(expected) is list and type(actual) is list:
        if len(expected) != len(actual):
            return [path + ': list length differs']
        return [error for index, (a, b) in enumerate(zip(expected, actual)) for error in close_values(a, b, path + '[' + str(index) + ']')]
    if type(expected) in (int, float) and type(actual) in (int, float):
        return [] if abs(expected - actual) <= 0.1 else [path + ': actual layout value differs']
    return [] if type(expected) is type(actual) and expected == actual else [path + ': actual value differs']


def recompute_scene_layout(root, bundle):
    report = bundle['report']
    directory = checked_ref(root, report['response_headers'], ignored=True).parent
    css_files = sorted((directory / 'bundle/assets').glob('*.css'))
    if not css_files or any(path.is_symlink() for path in css_files):
        raise ValueError('Original actual component build CSS assets missing')
    css = '\n'.join(path.read_text() for path in css_files)
    entries = []
    for scene in report.get('scenes', []):
        for index, step in enumerate(scene['observations']['steps']):
            entries.append({'id': scene['id'] + '/' + str(index),
                            'html': str(checked_ref(root, step['html'], ignored=True)),
                            'css': css, 'dom': step['dom'], 'source_details': step.get('source_details')})
    if not entries:
        return {'errors': [], 'executed_steps': 0, 'scope': 'no actual scene DOM available'}
    work = Path(tempfile.mkdtemp(prefix='independent-layout-', dir=directory))
    request = work / 'input.json'
    request.write_text(json.dumps(entries, ensure_ascii=False))
    command = ['node', '--input-type=module', '-e', LAYOUT_REPLAY, str(root), str(request)]
    started = datetime.now(timezone.utc).isoformat()
    completed = subprocess.run(command, cwd=root, text=True, capture_output=True, timeout=180)
    (work / 'stdout.json').write_text(completed.stdout)
    (work / 'stderr.log').write_text(completed.stderr)
    relative = work.relative_to(root).as_posix()
    record = {'kind': 'independent-binding-component-layout-execution/v1', 'command': command, 'cwd': '.',
              'started_at': started, 'finished_at': datetime.now(timezone.utc).isoformat(),
              'exit_code': completed.returncode, 'step_count': len(entries),
              'input': ref(root, relative + '/input.json'), 'stdout': ref(root, relative + '/stdout.json'),
              'stderr': ref(root, relative + '/stderr.log')}
    (work / 'execution.json').write_text(json.dumps(record, ensure_ascii=False, indent=2))
    if completed.returncode != 0:
        raise ValueError('Independent original-DOM locked Chromium reflow failed: ' + completed.stderr[-2000:])
    actual = json.loads(completed.stdout)
    if [row['id'] for row in actual] != [row['id'] for row in entries]:
        raise ValueError('Independent original-DOM reflow omitted/reordered checkpoints')
    errors = []
    for entry, observed in zip(entries, actual):
        for field in ('text', 'headings', 'dialogs', 'details', 'scrollOwners', 'buttons', 'checkboxes', 'previews', 'fonts', 'geometry'):
            errors.extend(close_values(entry['dom'][field], observed['actual'][field], entry['id'] + '.' + field))
        if 'progressStages' in entry['dom']:
            errors.extend(close_values(entry['dom']['progressStages'], observed['actual']['progressStages'], entry['id'] + '.progressStages'))
        if 'sourceRows' in entry['dom']:
            errors.extend(close_values(entry['dom']['sourceRows'], observed['actual']['sourceRows'], entry['id'] + '.sourceRows'))
        if entry.get('source_details'):
            for row in entry['source_details']['rows']:
                found = [value for value in observed['actual']['sourceDefinitions'] if value['sourceId'] == row['source_id']]
                if len(found) != row['rowCount']:
                    errors.append(entry['id'] + ': source row observation differs from original DOM')
                for field in row['fields']:
                    definitions = [value['value'] for value in found[0]['fields'] if value['label'] == field['label']] if len(found) == 1 else []
                    if len(definitions) != field['matchCount'] or (definitions[0] if len(definitions) == 1 else None) != field['actual']:
                        errors.append(entry['id'] + ': source dt/dd value differs from original DOM')
    return {'errors': errors, 'executed_steps': len(entries), 'execution': ref(root, relative + '/execution.json'),
            'scope': 'second locked Chromium; original retained DOM + actual CSS; no product scripts/network'}


def recompute(root, bundle):
    errors, judgments = [], []
    producer = bundle['producer']
    nonce = producer['values']['nonce']
    # Replaying a compiled production serializer is independent from trusting its
    # saved stdout. Its source and executable bytes were checked by inspect.
    try:
        result = subprocess.run([str(producer['binary']), nonce], cwd=root, text=True,
                                capture_output=True, timeout=30)
        if result.returncode != 0 or result.stderr.strip() or json.loads(result.stdout) != producer['values']:
            raise ValueError('Compiled production serializer replay differs from retained projection')
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        errors.append(str(error))
    for row in bundle['report']['cases']:
        trace = load(checked_ref(root, row['trace'], ignored=True))
        if producer.get('manifest', {}).get('runtime_transport') is not None:
            ready = bundle['actual_rpc'].get('ready')
            caps = ready['capabilities'] if ready else None
            methods = (*DIAGNOSTIC_METHODS[COMPONENT_CASES.index(row['id'])], 'controlWindowPreview')
            missing = (['producer-ready'] if caps is None else
                       ([] if DIAGNOSTIC_SCENES[row['id']] in caps['scene_ids'] else ['scene:' + DIAGNOSTIC_SCENES[row['id']]])
                       + ['method:' + method for method in dict.fromkeys(methods) if method not in caps['methods']]
                       + ([] if 'reset' in caps['controls'] else ['control:reset']))
            if missing:
                if (trace['observations'] != {'transport_mode': 'unavailable', 'blocked_obligations': missing}
                        or trace['events'] != [] or row.get('dom') is not None or row.get('screenshot') is not None
                        or row.get('status') != 'blocked' or row.get('assertions') != []):
                    errors.append(row['id'] + ': missing actual capability does not match retained diagnostic')
                judgments.append({'id': row['id'], 'status': 'blocked', 'blocked_obligations': missing})
                continue
        html = checked_ref(root, row['dom'], ignored=True).read_text()
        if row['id'] == COMPONENT_CASES[2]:
            try:
                captures = [load(checked_ref(root, value, ignored=True))
                            for value in trace['observations']['captureRecords']]
                final = captures[-1]
                before, after = final['before'], final['after']
                if (not 1 <= len(captures) <= 3
                        or any(value.get('nonce') != trace['nonce'] or value.get('case_id') != row['id']
                               or value.get('attempt') != number for number, value in enumerate(captures, 1))
                        or before['ui'] != after['ui']
                        or any(value['before']['ui'] == value['after']['ui'] for value in captures[:-1])
                        or before['facts']['events'] != trace['events']
                        or after['facts'] != trace['observations']['postCaptureFacts']
                        or after['facts']['events'][:len(trace['events'])] != trace['events']
                        or final['screenshot']['sha256'] != row['screenshot']['sha256']
                        or any(before['ui'][key] != trace['observations'][key]
                               for key in ('visibleText', 'stageLabels', 'duplicateSubmitAvailable'))):
                    raise ValueError('Actual pending capture borrowed another observation cut')
                for capture in captures:
                    checked_ref(root, capture['screenshot'], ignored=True)
                chain = [point['facts'] for capture in captures
                         for point in (capture['before'], capture['after'])]
                equal = lambda a, b: json.dumps(a, sort_keys=True) == json.dumps(b, sort_keys=True)
                mandatory = ('events', 'methodCalls', 'transportCalls', 'rpcMessages', 'associations')
                optional = ('commands', 'windows', 'previewStarts', 'pickers')
                for point in chain:
                    if any(not isinstance(point.get(key), list) for key in mandatory):
                        raise ValueError('Pending original cumulative fact inventory is missing')
                    calls = [event for event in point['events'] if event.get('kind') == 'port_call']
                    if (not equal(calls, point['methodCalls'])
                            or not equal([call['args'][0] for call in calls if call.get('method') == 'associate'], point['associations'])):
                        raise ValueError('Pending actual association inventory differs from original port calls')
                for prior, later in zip(chain, chain[1:]):
                    for key in (*mandatory, *(key for key in optional if key in prior or key in later)):
                        old, new = prior.get(key), later.get(key)
                        if not isinstance(old, list) or not isinstance(new, list) or not equal(new[:len(old)], old):
                            raise ValueError('Pending cumulative facts regress across capture cuts: ' + key)
            except (OSError, ValueError, KeyError, TypeError, IndexError) as error:
                errors.append(row['id'] + ': pending capture closure: ' + str(error))
        errors.extend(dom_errors(row['id'], trace['observations'], html, trace['events']))
        judgments.append(observation_verdict(row['id'], trace['observations']))
    for scene in bundle['report'].get('scenes', []):
        trace = load(checked_ref(root, scene['trace'], ignored=True))
        judgments.append(scene_verdict(scene['id'], trace['observations']))
        if scene['id'] in {'BJS-01-library-global-owner','BJS-01-scoped-reconnect-owner'} and trace['observations']['steps']:
            from binding_journey_inspect import inspect_library_owner_database
            try:
                inspect_library_owner_database(root, scene['id'], trace['observations']['steps'], bundle['actual_rpc'],
                                               checked_ref(root, scene['trace'], ignored=True).parent, nonce, bundle['source_map'])
            except (OSError, ValueError, KeyError, TypeError) as error:
                errors.append('Independent actual library owner database replay failed: ' + str(error))
    from binding_journey import scoped_source_discharge
    case = next((row for row in bundle['report']['cases'] if row['id'] == COMPONENT_CASES[4]), None)
    scene = next((row for row in bundle['report'].get('scenes', []) if row['id'] == 'BJS-10-details'), None)
    discharge = scoped_source_discharge(case, scene, nonce)
    if discharge != bundle['report'].get('source_detail_discharge'):
        errors.append('Same-run typed source detail discharge differs from original case/scene evidence')
    if discharge:
        from binding_journey_inspect import inspect_source_details
        if inspect_source_details(scene['observations']['steps'][-1], bundle['actual_rpc']) != []:
            errors.append('Same-run source discharge did not independently prove all precise typed source fields')
        else:
            for judgment in judgments:
                if judgment['id'] == COMPONENT_CASES[4] and judgment['status'] == 'blocked':
                    judgment.update(status='passed', retained_blocked_obligations=judgment['blocked_obligations'],
                                    blocked_obligations=[], discharged_by=discharge)
    layout = recompute_scene_layout(root, bundle)
    errors.extend(layout['errors'])
    rust_replay = bundle.get('rust_replay_disposition', 'required_current_tree')
    if rust_replay not in {'required_current_tree', 'not_run_historical_workspace_dependent'}:
        errors.append('Unknown historical Rust replay disposition')
    if bundle.get('rust_layers') and rust_replay == 'required_current_tree':
        try:
            from binding_journey_inspect import inspect_rust_layers
            replayed = inspect_rust_layers(root, bundle['rust_layers'], nonce, bundle['source_map'], replay=True)
            errors.extend(replayed['infrastructure_errors'])
        except (OSError, ValueError, KeyError, TypeError, subprocess.TimeoutExpired) as error:
            errors.append('Production Rust layer replay failed: ' + str(error))
    return {'schema_version': 1, 'kind': 'binding-journey-independent-recomputation/v1',
            'errors': errors, 'case_statuses': {x['id']: x['status'] for x in judgments},
            'blocked_obligations': sorted({b for x in judgments for b in x['blocked_obligations']}),
            'independent_component_layout': layout,
            'rust_replay': rust_replay,
            'product_acceptance_promoted': False, 'native_executed': False}
