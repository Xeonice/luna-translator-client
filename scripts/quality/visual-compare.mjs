import fs from 'node:fs';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
import {PNG} from 'pngjs';
import pixelmatch from 'pixelmatch';

export function compareImages({baseline, candidate, diff, maxRatio = 0}) {
  if (!Number.isFinite(maxRatio) || maxRatio < 0 || maxRatio > 1) throw new Error('Invalid visual threshold');
  const expected = PNG.sync.read(fs.readFileSync(baseline));
  const actual = PNG.sync.read(fs.readFileSync(candidate));
  if (expected.width !== actual.width || expected.height !== actual.height) {
    return {status: 'failed', reason: 'dimensions', expected: [expected.width, expected.height], actual: [actual.width, actual.height]};
  }
  const output = new PNG({width: actual.width, height: actual.height});
  const differentPixels = pixelmatch(expected.data, actual.data, output.data, actual.width, actual.height, {threshold: 0.1, includeAA: true});
  const ratio = differentPixels / (actual.width * actual.height);
  fs.mkdirSync(path.dirname(diff), {recursive: true});
  fs.writeFileSync(diff, PNG.sync.write(output));
  return {status: ratio <= maxRatio ? 'passed' : 'failed', differentPixels, ratio, maxRatio, diff};
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  try {
    const args = Object.fromEntries(process.argv.slice(2).reduce((rows, value, index, all) => {
      if (index % 2 === 0) rows.push([value.replace(/^--/, ''), all[index + 1]]);
      return rows;
    }, []));
    for (const key of ['baseline', 'candidate', 'diff']) if (!args[key]) throw new Error(`Missing --${key}`);
    const result = compareImages({...args, maxRatio: Number(args['max-ratio'] ?? 0)});
    process.stdout.write(JSON.stringify(result) + '\n');
    process.exitCode = result.status === 'passed' ? 0 : 1;
  } catch (error) { process.stderr.write(error.message + '\n'); process.exitCode = 2; }
}
