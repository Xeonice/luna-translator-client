// Independently recompute each emitted product PNG comparison; never mutate design references.
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {createRequire} from 'node:module';
const require = createRequire(path.resolve('package.json'));
const {PNG} = require('pngjs');
const pixelmatch = require('pixelmatch').default;
const reportPath = path.resolve(process.argv[2]);
const directory = path.dirname(reportPath);
const report = JSON.parse(fs.readFileSync(reportPath));
const rows = report.captures.flatMap(parent => [parent, ...(parent.variants ?? [])].filter(row => row.status !== 'blocked_pending_replacement_reference').map(row => {
  // Blocked obligations remain in the report and are rejected by the Python
  // inventory/status inspector. They have no PNGs and must never become zero
  // differences. Missing PNGs in every captured row still fail decoding.
  const expected = PNG.sync.read(fs.readFileSync(path.join(directory, row.baselineCopy)));
  const actual = PNG.sync.read(fs.readFileSync(path.join(directory, row.candidate)));
  const width = Math.max(expected.width, actual.width), height = Math.max(expected.height, actual.height);
  const before = new PNG({width, height}), after = new PNG({width, height});
  before.data.fill(0); after.data.fill(0);
  PNG.bitblt(expected, before, 0, 0, expected.width, expected.height, 0, 0);
  PNG.bitblt(actual, after, 0, 0, actual.width, actual.height, 0, 0);
  const diff = new PNG({width, height});
  const differentPixels = pixelmatch(before.data, after.data, diff.data, width, height, {threshold: 0.1, includeAA: true});
  return {id: parent.id, ...(row.variantId === undefined ? {} : {variantId: row.variantId}), expectedSize: [expected.width, expected.height], actualSize: [actual.width, actual.height], differentPixels, ratio: differentPixels / (width * height), diffSha256: createHash('sha256').update(PNG.sync.write(diff)).digest('hex')};
}));
process.stdout.write(JSON.stringify(rows) + '\n');
