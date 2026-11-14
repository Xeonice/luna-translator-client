import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { analyzeFrontend } from './frontend-boundaries.mjs';

function fixture(files, options = {}) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'luna-frontend-boundary-'));
  const entries = {
    'apps/desktop/tsconfig.json': JSON.stringify({ compilerOptions: { baseUrl: '.', paths: { '@/*': ['src/*'] }, ...options }, include: ['src'] }),
    ...Object.fromEntries(Object.entries(files).map(([name, content]) => [`apps/desktop/${name}`, content])),
  };
  for (const [name, content] of Object.entries(entries)) {
    const target = path.join(root, name);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, content);
  }
  return { root, dispose: () => fs.rmSync(root, { recursive: true, force: true }) };
}

function analyze(files, options) {
  const created = fixture(files, options);
  try { return analyzeFrontend({ root: created.root }); }
  finally { created.dispose(); }
}

const rules = result => result.findings.map(finding => finding.rule);

test('normal architecture passes, including configured aliases and private same-feature imports', () => {
  const result = analyze({
    'src/app/bootstrap.tsx': "import { port } from '@/bridge/tauri-port'; import { Screen } from '@/features/library'; void port; void Screen;",
    'src/bridge/tauri-port.ts': "import { invoke as call } from '@tauri-apps/api/core'; export const port = { query: () => call('snapshot') };",
    'src/bridge/application-port.ts': 'export interface ApplicationPort { query(): unknown; }',
    'src/bridge/contracts.generated.ts': 'export type Snapshot = { revision: number };',
    'src/features/library/index.ts': "export { Screen } from './Screen';",
    'src/features/library/Screen.tsx': "import type { ApplicationPort } from '@/bridge/application-port'; import { Card } from '@/ui/patterns/Card'; import { helper } from './private'; export const Screen = (port: ApplicationPort) => [port.query(), Card, helper];",
    'src/features/library/private.ts': 'export const helper = 1;',
    'src/ui/patterns/Card.tsx': "import { Button } from '../primitives/Button'; export const Card = Button;",
    'src/ui/primitives/Button.tsx': "import { Button as AriaButton } from 'react-aria-components'; import { icon } from '../icons/icon'; export const Button = [AriaButton, icon];",
    'src/ui/icons/icon.ts': 'export const icon = "path";',
  });
  assert.equal(result.status, 'pass', JSON.stringify(result.findings));
  assert.equal(result.scanned_files, 10);
});

test('comments, strings, property names, locally shadowed globals and type references do not create findings', () => {
  const result = analyze({
    'src/features/reading/index.ts': `
      // fetch('https://example.invalid'); import('axios');
      const documentation = "window.fetch and require('sqlite3')";
      const local = { fetch: () => 1, credentials: 'not system credentials' };
      export function example(fetch: () => number, window: { fetch(): number }, navigator: { credentials: string }) {
        return [fetch(), window.fetch(), navigator.credentials, local.fetch(), documentation];
      }
      export type Local = { fetch: string; credentials: string };
      export type Socket = WebSocket;
    `,
  });
  assert.equal(result.status, 'pass', JSON.stringify(result.findings));
});

test('global HTTP APIs, aliases, constructors, computed access and destructuring fail', () => {
  const result = analyze({
    'src/features/reading/index.ts': `
      fetch('/request');
      const request = fetch;
      const fetchAgain = window.fetch;
      globalThis['fetch']('/request');
      new WebSocket('wss://example.invalid');
      new window.XMLHttpRequest();
      const { fetch: indirectFetch } = self;
      const eventStream = new EventSource('/stream');
      const { indexedDB: database } = window;
      const credentials = navigator.credentials;
      const { credentials: vault } = navigator;
      const browser = window;
      browser.fetch('/request');
      navigator.sendBeacon('/request');
      const capabilities = { fetch };
      let assigned;
      ({ fetch: assigned } = window);
      const { credentials: windowVault } = window.navigator;
    `,
  });
  assert.equal(result.status, 'fail');
  assert.equal(result.findings.filter(finding => finding.rule === 'FE_HTTP_ACCESS').length, 12);
  assert.equal(result.findings.filter(finding => finding.rule === 'FE_DATABASE_ACCESS').length, 1);
  assert.equal(result.findings.filter(finding => finding.rule === 'FE_CREDENTIAL_ACCESS').length, 3);
});

test('static import aliases, require, dynamic import and re-exports of forbidden packages fail', () => {
  const result = analyze({
    'src/features/reading/index.ts': `
      import { default as request } from 'axios';
      export { Database } from 'better-sqlite3';
      const ffi = require('ffi-napi');
      const secrets = import('keytar');
      import type { Client } from 'openai';
      import fs = require('node:fs');
    `,
  });
  assert.deepEqual(rules(result).sort(), ['FE_HTTP_ACCESS', 'FE_HTTP_ACCESS', 'FE_DATABASE_ACCESS', 'FE_FFI_ACCESS', 'FE_CREDENTIAL_ACCESS', 'FE_NODE_IO'].sort());
});

test('Tauri imports are rejected outside the single gateway regardless of import style', () => {
  const result = analyze({
    'src/features/capture/index.ts': `
      import { invoke as call } from '@tauri-apps/api/core';
      const api = require('@tauri-apps/api/window');
      const api2 = import('@tauri-apps/plugin-http');
      window.__TAURI_INTERNALS__.invoke('raw');
      globalThis['__TAURI__'].core.invoke('raw');
    `,
  });
  assert.equal(result.findings.filter(finding => finding.rule === 'FE_TAURI_GATEWAY').length, 5);
});

test('features cannot bypass ApplicationPort by importing TauriPort', () => {
  const result = analyze({
    'src/features/capture/index.ts': "import { port } from '@/bridge/tauri-port'; export { port };",
    'src/bridge/tauri-port.ts': 'export const port = {};',
  });
  assert.deepEqual(rules(result), ['FE_BRIDGE_PRIVATE']);
});

test('cross-feature private access fails with TS aliases, relative paths and dynamic imports', () => {
  const result = analyze({
    'src/features/reading/index.ts': `
      import { detail } from '@/features/study/private';
      const detail2 = import('../study/private');
      type Detail = import('@/features/study/private').Detail;
      const detail3 = require('@/features/study/private');
    `,
    'src/features/study/index.ts': "export { detail } from './private';",
    'src/features/study/private.ts': 'export const detail = 1; export type Detail = number;',
  });
  assert.equal(result.findings.length, 4);
  assert.ok(rules(result).every(rule => rule === 'FE_FEATURE_PRIVATE_IMPORT'));
});

test('UI/business layer inversions and Aria usage outside primitives fail', () => {
  const result = analyze({
    'src/ui/patterns/Card.tsx': "import { business } from '@/features/study'; import { port } from '@/bridge/application-port';",
    'src/ui/primitives/Button.tsx': "import { Card } from '../patterns/Card';",
    'src/bridge/projections/index.ts': "import { business } from '@/features/study';",
    'src/features/study/index.ts': "import { Window } from '@/windows/main/MainWindow'; import { Button } from 'react-aria-components'; export const business = 1;",
    'src/bridge/application-port.ts': 'export const port = {};',
    'src/windows/main/MainWindow.tsx': 'export const Window = 1;',
  });
  assert.deepEqual(rules(result).sort(), ['FE_UI_BUSINESS_IMPORT', 'FE_UI_BUSINESS_IMPORT', 'FE_UI_LAYER_ORDER', 'FE_BRIDGE_UI_IMPORT', 'FE_FEATURE_UPWARD_IMPORT', 'FE_ARIA_WRAPPER'].sort());
});

test('nonliteral module paths, glob and generated code fail closed', () => {
  const result = analyze({
    'src/features/study/index.ts': `
      const name = 'axios';
      import(name);
      require(name);
      import.meta.glob('../**/*.ts');
      eval('fetch(\"/request\")');
      const create = new Function('return 1');
    `,
  });
  assert.deepEqual(rules(result).sort(), ['FE_DYNAMIC_MODULE', 'FE_DYNAMIC_MODULE', 'FE_DYNAMIC_MODULE', 'FE_DYNAMIC_CODE', 'FE_DYNAMIC_CODE'].sort());
});

test('unresolved local alias cannot bypass boundary checks', () => {
  const result = analyze({ 'src/features/study/index.ts': "import '@/features/missing/private';" });
  assert.deepEqual(rules(result), ['FE_UNRESOLVED_LOCAL_IMPORT']);
});

test('production import of test support fails', () => {
  const result = analyze({
    'src/app/bootstrap.tsx': "import { fixture } from '../../tests/support/design-port'; void fixture;",
    'tests/support/design-port.ts': 'export const fixture = {};',
  });
  assert.deepEqual(rules(result).sort(), ['FE_PRODUCTION_TEST_IMPORT', 'FE_SOURCE_ESCAPE'].sort());
});

test('unknown source layers and imports of legacy root modules cannot bypass dependency checks', () => {
  const result = analyze({
    'src/App.tsx': 'export const legacy = 1;',
    'src/services/business.ts': 'export const business = 2;',
    'src/features/study/index.ts': "import { legacy } from '../../App'; export { legacy };",
  });
  assert.deepEqual(rules(result).sort(), ['FE_SOURCE_LAYOUT', 'FE_SOURCE_LAYOUT', 'FE_UNCLASSIFIED_IMPORT'].sort());
});

test('malformed source is a failing finding, not silently skipped', () => {
  const result = analyze({ 'src/app/bootstrap.tsx': 'export const value = ;' });
  assert.ok(rules(result).includes('FE_PARSE_ERROR'));
});

test('CLI JSON is portable and nonzero on violation; missing source fails configuration', () => {
  const created = fixture({ 'src/features/study/index.ts': 'fetch("/request");' });
  const checker = fileURLToPath(new URL('./frontend-boundaries.mjs', import.meta.url));
  try {
    const result = spawnSync(process.execPath, [checker, '--root', created.root, '--json'], { encoding: 'utf8' });
    assert.equal(result.status, 1, result.stderr);
    const json = JSON.parse(result.stdout);
    assert.equal(json.findings[0].path, 'apps/desktop/src/features/study/index.ts');
    assert.equal(json.findings[0].line, 1);
    assert.equal(json.findings[0].rule, 'FE_HTTP_ACCESS');
    assert.ok(!result.stdout.includes(created.root));
    fs.rmSync(path.join(created.root, 'apps/desktop/src'), { recursive: true });
    const missing = spawnSync(process.execPath, [checker, '--root', created.root, '--json'], { encoding: 'utf8' });
    assert.equal(missing.status, 2);
    assert.match(missing.stderr, /Missing required source directory/);
  } finally { created.dispose(); }
});
