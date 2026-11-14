// Historical source provenance only. No capture, rendering, writes or fallback.
import fs from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';

export const MANIFEST_PATH = 'docs/development/design-source-version-v4.json';
export const CATALOGUE_PATH = 'docs/design/first-product-v1/acceptance-v4/catalogue.json';
export const APPROVAL_PATH = 'docs/development/design-approval-20261019.json';
export const BASELINE_PATH = 'docs/development/iteration-baseline.json';
export const VERSION_ID = 'reviewed-v4-original-sources-v1';
export const PURPOSE = 'historical_reference_source_resolution_only';
const design = 'docs/design/first-product-v1/';
export const ALLOWED_RELOCATIONS = Object.freeze(Object.fromEntries(
  ['app.js', 'game-library.js', 'review-gaps.css', 'review-gaps.js', 'README.md',
    'data.js', 'index.html', 'coverage.csv', 'integration.md', 'manifest.json', 'style-guide.md']
    .map(name => [design + name, design + 'acceptance-v4/design-origin/' + name]),
));
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const exactKeys = (value, keys) => object(value) && Object.keys(value).sort().join('\n') === [...keys].sort().join('\n');

export function parseUniqueJson(raw) {
  const text = Buffer.isBuffer(raw) ? new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(raw) : raw;
  const value = JSON.parse(text); // Grammar validation precedes duplicate-key scan.
  let index = 0;
  const whitespace = () => { while (/\s/.test(text[index] ?? '') && index < text.length) index++; };
  const string = () => {
    const start = index++;
    while (index < text.length) {
      if (text[index] === '\\') { index += 2; continue; }
      if (text[index++] === '"') return JSON.parse(text.slice(start, index));
    }
    throw new Error('Unterminated source-version JSON string');
  };
  const scan = depth => {
    assert.ok(depth <= 128, 'Source-version JSON nesting is excessive');
    whitespace();
    if (text[index] === '{') {
      index++; whitespace(); const keys = new Set();
      if (text[index] !== '}') while (true) {
        whitespace(); const key = string();
        assert.ok(!keys.has(key), `Duplicate source-version JSON key: ${key}`); keys.add(key);
        whitespace(); index++; scan(depth + 1); whitespace();
        if (text[index] !== ',') break;
        index++;
      }
      index++;
    } else if (text[index] === '[') {
      index++; whitespace();
      if (text[index] !== ']') while (true) { scan(depth + 1); whitespace(); if (text[index] !== ',') break; index++; }
      index++;
    } else if (text[index] === '"') string();
    else while (index < text.length && !/[\s,\]}]/.test(text[index])) index++;
  };
  scan(0); whitespace(); assert.equal(index, text.length, 'Incomplete source-version JSON scan');
  return value;
}

export async function safeFile(root, relative) {
  assert.ok(typeof relative === 'string' && relative.length && relative.trim() === relative
    && !/[\x00-\x1f\x7f\\:?#]/.test(relative) && !path.posix.isAbsolute(relative)
    && relative.split('/').every(part => part !== '' && part !== '.' && part !== '..'),
  `Unsafe source-version path: ${String(relative)}`);
  let current = root;
  for (const part of relative.split('/')) {
    current = path.join(current, part);
    assert.ok(!(await fs.lstat(current)).isSymbolicLink(), `Source-version symlink is forbidden: ${relative}`);
  }
  assert.ok((await fs.stat(current)).isFile(), `Missing source-version file: ${relative}`);
  assert.ok(!path.relative(root, await fs.realpath(current)).startsWith('..'), `Escaped source-version file: ${relative}`);
  return current;
}

async function pinnedBytes(root, baseline, relative) {
  const raw = await fs.readFile(await safeFile(root, relative));
  const pins = baseline.files.filter(row => object(row) && row.path === relative);
  assert.ok(pins.length === 1 && pins[0].sha256 === hash(raw)
    && (!Object.hasOwn(pins[0], 'bytes') || pins[0].bytes === raw.length),
  `Source-version input lacks an exact independent authority pin: ${relative}`);
  return raw;
}

export async function loadDesignSourceVersion(root, catalogue, catalogueSha256) {
  root = path.resolve(root);
  assert.ok(!(await fs.lstat(root)).isSymbolicLink(), 'Source-version repository root may not be a symlink');
  root = await fs.realpath(root);
  const baseline = parseUniqueJson(await fs.readFile(await safeFile(root, BASELINE_PATH)));
  assert.ok(object(baseline) && Array.isArray(baseline.files), 'Invalid source-version authority baseline');
  const manifestRaw = await pinnedBytes(root, baseline, MANIFEST_PATH), manifest = parseUniqueJson(manifestRaw);
  const catalogueRaw = await pinnedBytes(root, baseline, CATALOGUE_PATH), actualCatalogue = parseUniqueJson(catalogueRaw);
  const approvalRaw = await pinnedBytes(root, baseline, APPROVAL_PATH), approval = parseUniqueJson(approvalRaw);
  assert.ok(object(actualCatalogue) && actualCatalogue.schemaVersion === 4
    && actualCatalogue.status === 'registered_independently_reviewed', 'Source-version catalogue/profile identity differs');
  if (catalogue !== undefined) assert.deepEqual(catalogue, actualCatalogue, 'Source-version catalogue object differs');
  if (catalogueSha256 !== undefined) assert.equal(catalogueSha256, hash(catalogueRaw), 'Source-version catalogue hash differs');
  assert.ok(exactKeys(manifest, ['schema_version', 'version_id', 'purpose', 'catalogue', 'approval', 'entries'])
    && manifest.schema_version === 1 && manifest.version_id === VERSION_ID && manifest.purpose === PURPOSE,
  'Source-version manifest version contract differs');
  assert.deepEqual(manifest.catalogue, { path: CATALOGUE_PATH, sha256: hash(catalogueRaw) }, 'Source-version catalogue identity differs');
  assert.deepEqual(manifest.approval, { path: APPROVAL_PATH, sha256: hash(approvalRaw) }, 'Source-version approval identity differs');
  const frozen = actualCatalogue.frozenFiles, approved = approval?.source_files_sha256;
  assert.ok(object(frozen) && Object.keys(frozen).length > 0 && object(approved)
    && Object.keys(approved).length === 16, 'Source-version original approved source closure differs');
  assert.deepEqual(actualCatalogue.approvedDesignSources, approved, 'Source-version approved sources differ');
  assert.ok(Object.entries(approved).every(([file, sha]) => frozen[file] === sha), 'Source-version approved source hash differs');
  assert.ok(Array.isArray(manifest.entries) && manifest.entries.length === Object.keys(frozen).length,
    'Source-version mapping must cover the exact complete logical domain');
  const paths = new Map(), contents = new Map(), resolutions = [];
  for (const row of manifest.entries) {
    assert.ok(exactKeys(row, ['logical_path', 'physical_path', 'sha256', 'bytes']), 'Invalid source-version mapping row');
    const { logical_path: logical, physical_path: physical, sha256: expected, bytes: size } = row;
    assert.ok(typeof logical === 'string' && !paths.has(logical) && Object.hasOwn(frozen, logical)
      && (logical.startsWith(design) || logical.startsWith('apps/desktop/tests/visual/'))
      && /^[0-9a-f]{64}$/.test(expected) && frozen[logical] === expected
      && Number.isSafeInteger(size) && size > 0, `Source-version logical identity/domain differs: ${String(logical)}`);
    await safeFile(root, logical);
    const target = await safeFile(root, physical);
    assert.ok(physical === logical || ALLOWED_RELOCATIONS[logical] === physical && frozen[physical] === expected,
      `Unapproved source-version relocation: ${logical}`);
    if (!contents.has(physical)) contents.set(physical, await fs.readFile(target));
    const bytes = contents.get(physical);
    assert.ok(hash(bytes) === expected && bytes.length === size, `Historical source-version physical bytes differ: ${logical}`);
    paths.set(logical, physical);
    resolutions.push({ logical_path: logical, physical_path: physical, sha256: expected, bytes: size });
  }
  assert.deepEqual([...paths.keys()].sort(), Object.keys(frozen).sort(), 'Source-version logical domain differs');
  assert.ok([...paths.values()].every(physical => paths.get(physical) === physical), 'Source-version one-level mapping differs');
  const activeDesignSourceHashes = Object.fromEntries(await Promise.all(Object.keys(approved)
    .map(async file => [file, hash(await fs.readFile(await safeFile(root, file)))])));
  return {
    resolve: logical => { assert.ok(paths.has(logical), `Unregistered logical source-version input: ${logical}`); return path.join(root, paths.get(logical)); },
    receipt: { manifestPath: MANIFEST_PATH, manifestSha256: hash(manifestRaw), versionId: VERSION_ID,
      catalogueSha256: hash(catalogueRaw), approvalSha256: hash(approvalRaw), resolutions, activeDesignSourceHashes },
  };
}
