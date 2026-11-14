// Guard-only byte fixtures. These are neither product PNGs nor UI acceptance.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { MANIFEST_PATH, CATALOGUE_PATH, APPROVAL_PATH, BASELINE_PATH, VERSION_ID, PURPOSE,
  ALLOWED_RELOCATIONS, loadDesignSourceVersion, parseUniqueJson } from './design-source-versions.mjs';
import { loadReferenceCatalogue } from './capture-catalogue.mjs';

const project = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../..');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const python = `import sys,json;sys.path.insert(0,sys.argv[1]);from design_source_versions import load_source_version;print(json.dumps(load_source_version(sys.argv[2]).receipt))`;
const pythonResult = root => spawnSync('python3', ['-c', python, path.join(project, 'scripts/quality'), root], { encoding: 'utf8' });
const design = 'docs/design/first-product-v1/';
const legacyAliasNames = ['app.js', 'game-library.js', 'review-gaps.css', 'review-gaps.js', 'README.md'];
const addedAliasNames = ['data.js', 'index.html', 'coverage.csv', 'integration.md', 'manifest.json', 'style-guide.md'];
const legacyRelocations = Object.fromEntries(legacyAliasNames.map(name => [design + name,
  design + 'acceptance-v4/design-origin/' + name]));

async function fixture(t, { aliases = true, aliasNames = legacyAliasNames } = {}) {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'luna-source-version-'));
  t.after(() => fs.rm(root, { recursive: true, force: true }));
  const f = { root, frozen: {}, approved: {}, aliases: {}, baseline: { files: [] } };
  f.write = async (relative, bytes) => {
    const file = path.join(root, relative); await fs.mkdir(path.dirname(file), { recursive: true }); await fs.writeFile(file, bytes); return file;
  };
  f.freeze = async (relative, bytes) => { await f.write(relative, bytes); f.frozen[relative] = hash(bytes); };
  f.pin = async relative => {
    const bytes = await fs.readFile(path.join(root, relative));
    f.baseline.files = f.baseline.files.filter(row => row.path !== relative);
    f.baseline.files.push({ path: relative, sha256: hash(bytes), bytes: bytes.length });
    await f.write(BASELINE_PATH, JSON.stringify(f.baseline));
  };
  f.writeManifest = async (manifest = f.manifest, raw = null) => {
    f.manifest = manifest;
    await f.write(MANIFEST_PATH, raw ?? JSON.stringify(manifest)); await f.pin(MANIFEST_PATH);
  };
  f.save = async () => {
    f.catalogue = { schemaVersion: 4, status: 'registered_independently_reviewed',
      fixturePurpose: 'guard_only_not_product_acceptance', pixelmatch: { threshold: .1, includeAA: true, maxRatio: 0 },
      approvedDesignSources: f.approved, frozenFiles: f.frozen, states: [] };
    await f.write(CATALOGUE_PATH, JSON.stringify(f.catalogue)); await f.pin(CATALOGUE_PATH);
    await f.write(APPROVAL_PATH, JSON.stringify({ source_files_sha256: f.approved })); await f.pin(APPROVAL_PATH);
    f.manifest = { schema_version: 1, version_id: VERSION_ID, purpose: PURPOSE,
      catalogue: { path: CATALOGUE_PATH, sha256: hash(await fs.readFile(path.join(root, CATALOGUE_PATH))) },
      approval: { path: APPROVAL_PATH, sha256: hash(await fs.readFile(path.join(root, APPROVAL_PATH))) }, entries: [] };
    for (const logical of Object.keys(f.frozen).sort()) {
      const physical = f.aliases[logical] ?? logical;
      f.manifest.entries.push({ logical_path: logical, physical_path: physical,
        sha256: f.frozen[logical], bytes: (await fs.stat(path.join(root, physical))).size });
    }
    await f.writeManifest();
  };
  // The legacy fixture still exercises exactly five aliases. Keep its original
  // approved closure at sixteen when the separate permitted path set grows.
  const files = [...Object.keys(ALLOWED_RELOCATIONS), ...Array.from({ length: 16 - Object.keys(ALLOWED_RELOCATIONS).length }, (_, i) => `docs/design/first-product-v1/guard-${i}.js`)];
  for (const logical of files) {
    const old = Buffer.from('Guard-only original design: ' + logical);
    await f.freeze(logical, old); f.approved[logical] = f.frozen[logical];
    if (aliases && aliasNames.some(name => design + name === logical)) {
      const physical = ALLOWED_RELOCATIONS[logical]; await f.freeze(physical, old); f.aliases[logical] = physical;
      await f.write(logical, 'Guard-only later active design: ' + logical);
    }
  }
  f.png = 'docs/design/first-product-v1/acceptance-v4/guard-only.png';
  f.producer = 'apps/desktop/tests/visual/prepare-reviewed-references.mjs';
  f.adapter = 'docs/design/first-product-v1/acceptance-v4/guard-only-adapter.json';
  await f.freeze(f.png, Buffer.from('Guard-only bytes, explicitly not an image'));
  await f.freeze(f.producer, Buffer.from('Guard-only independent producer'));
  await f.freeze(f.adapter, Buffer.from('{"fixture":"guard-only"}'));
  await f.save();
  return f;
}

async function rejectsInBoth(f, pattern = undefined) {
  await assert.rejects(() => loadDesignSourceVersion(f.root), pattern);
  const p = pythonResult(f.root);
  assert.notEqual(p.status, 0, 'Independent Python must also reject the same fixture');
}

test('same-byte five aliases preserve logical identities and distinct active sources in both languages', async t => {
  const f = await fixture(t), version = await loadDesignSourceVersion(f.root);
  const p = pythonResult(f.root); assert.equal(p.status, 0, p.stderr);
  assert.deepEqual(version.receipt, JSON.parse(p.stdout));
  assert.equal(version.receipt.resolutions.filter(row => row.logical_path !== row.physical_path).length, 5);
  for (const [logical, physical] of Object.entries(legacyRelocations)) {
    assert.equal(version.resolve(logical), version.resolve(physical), 'Physical sharing with the already-frozen origin is legitimate');
    assert.equal(hash(await fs.readFile(version.resolve(logical))), f.frozen[logical]);
    assert.notEqual(version.receipt.activeDesignSourceHashes[logical], f.frozen[logical]);
  }
  const catalogue = await loadReferenceCatalogue(CATALOGUE_PATH, { root: f.root });
  assert.deepEqual(catalogue.designSourceVersion, version.receipt);
  assert.equal(catalogue.sha256, f.manifest.catalogue.sha256);
  assert.equal(catalogue.resolveReference(f.png), path.join(await fs.realpath(f.root), f.png));
});

test('manifest must have exactly one independent pin and exact bytes', async t => {
  for (const mode of ['missing', 'duplicate', 'wrong_hash', 'wrong_size']) await t.test(mode, async child => {
    const f = await fixture(child), pin = f.baseline.files.find(row => row.path === MANIFEST_PATH);
    if (mode === 'missing') f.baseline.files = f.baseline.files.filter(row => row !== pin);
    else if (mode === 'duplicate') f.baseline.files.push({ ...pin });
    else if (mode === 'wrong_hash') pin.sha256 = '0'.repeat(64);
    else pin.bytes++;
    await f.write(BASELINE_PATH, JSON.stringify(f.baseline));
    await rejectsInBoth(f, /independent authority pin/);
  });
});

test('wrong version, catalogue, approval, schema and fallback options are refused', async t => {
  for (const mode of ['version', 'catalogue', 'approval', 'schema', 'fallback']) await t.test(mode, async child => {
    const f = await fixture(child), m = structuredClone(f.manifest);
    if (mode === 'version') m.version_id = 'unreviewed-version';
    if (mode === 'catalogue') m.catalogue.sha256 = '0'.repeat(64);
    if (mode === 'approval') m.approval.sha256 = '0'.repeat(64);
    if (mode === 'schema') m.schema_version = true;
    if (mode === 'fallback') m.fallback = true;
    await f.writeManifest(m); await rejectsInBoth(f, /version|catalogue|approval/);
  });
});

test('missing, extra, duplicate logical rows and incorrect logical hash are refused', async t => {
  for (const mode of ['missing', 'extra', 'duplicate', 'hash']) await t.test(mode, async child => {
    const f = await fixture(child), m = structuredClone(f.manifest);
    if (mode === 'missing') m.entries.pop();
    if (mode === 'extra') m.entries.push({ ...m.entries[0], logical_path: 'docs/design/first-product-v1/unregistered.js' });
    if (mode === 'duplicate') m.entries[m.entries.length - 1] = structuredClone(m.entries[0]);
    if (mode === 'hash') m.entries[0].sha256 = '0'.repeat(64);
    await f.writeManifest(m); await rejectsInBoth(f);
  });
});

test('reference PNGs, independent producers and adapters cannot be redirected even to same bytes', async t => {
  for (const mode of ['png', 'producer', 'adapter']) await t.test(mode, async child => {
    const f = await fixture(child), logical = f[mode];
    const physical = 'docs/design/first-product-v1/acceptance-v4/guard-copy.bin';
    await f.freeze(physical, await fs.readFile(path.join(f.root, logical))); await f.save();
    const m = structuredClone(f.manifest); m.entries.find(row => row.logical_path === logical).physical_path = physical;
    await f.writeManifest(m); await rejectsInBoth(f, /Unapproved source-version relocation/);
  });
});

test('absolute, traversal, network, backslash, empty and hidden backup paths are refused', async t => {
  for (const physical of ['/tmp/source.js', '../escape.js', 'https://invalid/source.js', 'C:\\source.js',
    'docs//source.js', 'docs/./source.js', '.build/unlisted-backup.js', 'docs/\u0000source.js']) {
    await t.test(JSON.stringify(physical), async child => {
      const f = await fixture(child), m = structuredClone(f.manifest); m.entries[0].physical_path = physical;
      if (physical === '.build/unlisted-backup.js') await f.write(physical,
        await fs.readFile(path.join(f.root, f.manifest.entries[0].physical_path)));
      await f.writeManifest(m); await rejectsInBoth(f);
    });
  }
});

test('origin tamper or absence cannot fall back to valid active bytes', async t => {
  for (const mode of ['tamper', 'missing', 'size']) await t.test(mode, async child => {
    const f = await fixture(child), [logical, physical] = Object.entries(ALLOWED_RELOCATIONS)[0];
    await f.write(logical, await fs.readFile(path.join(f.root, physical)));
    if (mode === 'tamper') await f.write(physical, 'wrong-history');
    if (mode === 'missing') await fs.unlink(path.join(f.root, physical));
    if (mode === 'size') {
      const m = structuredClone(f.manifest); m.entries.find(row => row.logical_path === logical).bytes++;
      await f.writeManifest(m);
    }
    await rejectsInBoth(f);
  });
});

test('symlinked origin files, ancestors and repository roots are refused', async t => {
  for (const mode of ['file', 'parent', 'root']) await t.test(mode, async child => {
    const f = await fixture(child), physical = Object.values(ALLOWED_RELOCATIONS)[0];
    if (mode === 'file') {
      const bytes = await fs.readFile(path.join(f.root, physical)); await fs.unlink(path.join(f.root, physical));
      await f.write('alternate.js', bytes); await fs.symlink(path.join(f.root, 'alternate.js'), path.join(f.root, physical));
    } else if (mode === 'parent') {
      const parent = path.dirname(path.join(f.root, physical)); await fs.rename(parent, parent + '-actual'); await fs.symlink(parent + '-actual', parent);
    } else {
      const link = f.root + '-link'; await fs.symlink(f.root, link); child.after(() => fs.unlink(link)); f.root = link;
    }
    await rejectsInBoth(f, /symlink/);
  });
});

test('identity bytes and mapping chains cannot be repaired by the resolver', async t => {
  await t.test('identity', async child => {
    const f = await fixture(child);
    await f.write('.build/guard-only.png', await fs.readFile(path.join(f.root, f.png)));
    await f.write(f.png, 'changed-identity-bytes'); await rejectsInBoth(f, /physical bytes differ/);
  });
  await t.test('chain', async child => {
    const f = await fixture(child), [logical, physical] = Object.entries(ALLOWED_RELOCATIONS)[0];
    const m = structuredClone(f.manifest); m.entries.find(row => row.logical_path === physical).physical_path = logical;
    await f.writeManifest(m); await rejectsInBoth(f, /Unapproved source-version relocation/);
  });
});

test('duplicate root, nested and escaped JSON keys and invalid UTF8 are refused', async t => {
  for (const mode of ['root', 'nested', 'escaped', 'invalid_utf8']) await t.test(mode, async child => {
    const f = await fixture(child), original = JSON.stringify(f.manifest);
    const raw = mode === 'root' ? '{"purpose":"ignored",' + original.slice(1)
      : mode === 'nested' ? original.replace('"physical_path":', '"physical_path":"ignored","physical_path":')
        : mode === 'escaped' ? original.replace('"version_id":', '"version_\\u0069d":"ignored","version_id":')
          : Buffer.concat([Buffer.from([255]), Buffer.from(original)]);
    await f.writeManifest(f.manifest, raw); await rejectsInBoth(f);
  });
  assert.throws(() => parseUniqueJson('{"a":1,"\\u0061":2}'), /Duplicate/);
});

test('integral JSON numeric spellings have identical typed meaning in both runtimes', async t => {
  const f = await fixture(t), size = f.manifest.entries[0].bytes;
  const raw = JSON.stringify(f.manifest).replace('"schema_version":1', '"schema_version":1e0')
    .replace(`"bytes":${size}`, `"bytes":${size}.0`);
  await f.writeManifest(f.manifest, raw);
  const actual = await loadDesignSourceVersion(f.root), p = pythonResult(f.root);
  assert.equal(p.status, 0, p.stderr); assert.deepEqual(actual.receipt, JSON.parse(p.stdout));
});

test('proposal and v3 profiles never borrow the reviewed v4 historical map', async t => {
  const f = await fixture(t);
  await assert.rejects(() => loadReferenceCatalogue(CATALOGUE_PATH, { root: f.root, proposalPreview: true }), /Frozen design input changed/);
  await assert.rejects(() => loadReferenceCatalogue('another-catalogue.json', { root: f.root }));
  const v3 = await fixture(t, { aliases: false });
  const catalogue = { ...v3.catalogue, schemaVersion: 3 };
  const v3Path = 'docs/design/first-product-v1/acceptance-v3/catalogue.json';
  await v3.write(v3Path, JSON.stringify(catalogue)); await fs.unlink(path.join(v3.root, MANIFEST_PATH));
  const loaded = await loadReferenceCatalogue(v3Path, { root: v3.root });
  assert.equal(loaded.designSourceVersion, null); assert.equal(loaded.schemaVersion, 3);
});

test('candidate manifest resolves all original 306 inputs with exactly eleven audited same-byte aliases including the original five', async () => {
  const m = JSON.parse(await fs.readFile(path.join(project, MANIFEST_PATH), 'utf8'));
  const catalogueRaw = await fs.readFile(path.join(project, CATALOGUE_PATH)), catalogue = JSON.parse(catalogueRaw);
  const approvalRaw = await fs.readFile(path.join(project, APPROVAL_PATH));
  assert.equal(m.catalogue.sha256, hash(catalogueRaw)); assert.equal(m.approval.sha256, hash(approvalRaw));
  assert.equal(m.entries.length, 306); assert.deepEqual(m.entries.map(row => row.logical_path).sort(), Object.keys(catalogue.frozenFiles).sort());
  const aliases = m.entries.filter(row => row.logical_path !== row.physical_path);
  assert.equal(aliases.filter(row => Object.hasOwn(legacyRelocations, row.logical_path)).length, 5);
  assert.equal(aliases.length, 11); assert.deepEqual(Object.fromEntries(aliases.map(row => [row.logical_path, row.physical_path])), ALLOWED_RELOCATIONS);
  for (const row of m.entries) {
    assert.equal(row.sha256, catalogue.frozenFiles[row.logical_path]);
    const bytes = await fs.readFile(path.join(project, row.physical_path));
    assert.equal(hash(bytes), row.sha256); assert.equal(bytes.length, row.bytes);
  }
  // This checks candidate bytes only; it does not claim baseline adoption or pixels.
});

for (const name of addedAliasNames) {
  test(`new exact same-byte alias ${name} resolves independently in both languages`, async t => {
    const f = await fixture(t, { aliasNames: [name] }), logical = design + name;
    const physical = design + 'acceptance-v4/design-origin/' + name;
    const version = await loadDesignSourceVersion(f.root), p = pythonResult(f.root);
    assert.equal(p.status, 0, p.stderr); assert.deepEqual(version.receipt, JSON.parse(p.stdout));
    assert.equal(version.receipt.resolutions.filter(row => row.logical_path !== row.physical_path).length, 1);
    assert.equal(version.resolve(logical), version.resolve(physical));
    assert.equal(hash(await fs.readFile(version.resolve(logical))), f.frozen[logical]);
    assert.notEqual(version.receipt.activeDesignSourceHashes[logical], f.frozen[logical]);
  });
}

test('all eleven exact aliases coexist without changing the sixteen approved source identities', async t => {
  const f = await fixture(t, { aliasNames: [...legacyAliasNames, ...addedAliasNames] });
  const version = await loadDesignSourceVersion(f.root), p = pythonResult(f.root);
  assert.equal(p.status, 0, p.stderr); assert.deepEqual(version.receipt, JSON.parse(p.stdout));
  assert.equal(Object.keys(f.approved).length, 16);
  assert.equal(version.receipt.resolutions.filter(row => row.logical_path !== row.physical_path).length, 11);
  for (const [logical, physical] of Object.entries(ALLOWED_RELOCATIONS)) {
    assert.equal(version.resolve(logical), version.resolve(physical));
    assert.equal(hash(await fs.readFile(version.resolve(logical))), f.frozen[logical]);
  }
});

test('a twelfth same-name design-origin alias remains unapproved even with frozen exact bytes', async t => {
  const f = await fixture(t, { aliasNames: [...legacyAliasNames, ...addedAliasNames] }), logical = design + 'controls.js';
  const physical = design + 'acceptance-v4/design-origin/controls.js';
  const original = Buffer.from('Guard-only unapproved twelfth alias');
  await f.freeze(logical, original); await f.freeze(physical, original);
  f.aliases[logical] = physical; await f.save();
  assert.equal(f.manifest.entries.filter(row => row.logical_path !== row.physical_path).length, 12);
  assert.equal(Object.keys(f.approved).length, 16);
  await rejectsInBoth(f, /Unapproved source-version relocation/);
});

test('each added alias refuses wrong bytes, sizes, chains and escaped paths', async t => {
  for (const name of addedAliasNames) for (const mode of ['bytes', 'size', 'chain', 'escape']) {
    await t.test(name + '/' + mode, async child => {
      const f = await fixture(child, { aliasNames: [name] }), logical = design + name;
      const physical = design + 'acceptance-v4/design-origin/' + name;
      const m = structuredClone(f.manifest);
      if (mode === 'bytes') {
        // Valid active bytes cannot rescue a damaged explicitly selected origin.
        await f.write(logical, await fs.readFile(path.join(f.root, physical)));
        await f.write(physical, 'wrong-history-for-' + name);
      } else if (mode === 'size') m.entries.find(row => row.logical_path === logical).bytes++;
      else if (mode === 'chain') m.entries.find(row => row.logical_path === physical).physical_path = logical;
      else m.entries.find(row => row.logical_path === logical).physical_path = '../escaped-' + name;
      if (mode !== 'bytes') await f.writeManifest(m);
      await rejectsInBoth(f, mode === 'bytes' || mode === 'size' ? /physical bytes differ/
        : mode === 'chain' ? /Unapproved source-version relocation/ : /Unsafe source-version path/);
    });
  }
});
