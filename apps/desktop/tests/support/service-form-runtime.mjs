/** Test-only local transport. All field policy is executed by the real Rust Registry. */
import fs from 'node:fs/promises';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { createHash } from 'node:crypto';
import { createInterface } from 'node:readline';

const sha = bytes => createHash('sha256').update(bytes).digest('hex');
export async function startServiceFormRuntime({ root, output }) {
  const compile = spawn('cargo', ['build', '-p', 'app-infra', '--example', 'service_form_fixture', '--locked', '--offline'], { cwd: root, stdio: ['ignore', 'pipe', 'pipe'] });
  let stdout = '', stderr = '';
  compile.stdout.on('data', data => stdout += data); compile.stderr.on('data', data => stderr += data);
  const exitCode = await new Promise((resolve, reject) => { compile.on('error', reject); compile.on('close', resolve); });
  await fs.writeFile(path.join(output, 'rust-fixture-build.json'), JSON.stringify({ command: ['cargo', 'build', '-p', 'app-infra', '--example', 'service_form_fixture', '--locked', '--offline'], exitCode, stdout, stderr }, null, 2) + '\n');
  if (exitCode !== 0) throw new Error('Actual Rust form fixture did not compile; see rust-fixture-build.json.');
  const binary = path.join(root, 'target/debug/examples/service_form_fixture');
  const binarySha256 = sha(await fs.readFile(binary));
  const child = spawn(binary, [], { cwd: root, stdio: ['pipe', 'pipe', 'pipe'] });
  const waiting = []; let fixtureError = '', closed = false;
  child.stderr.on('data', data => { fixtureError = (fixtureError + data).slice(-16_384); });
  const reader = createInterface({ input: child.stdout });
  reader.on('line', line => { const next = waiting.shift(); if (!next) return; try { next.resolve(JSON.parse(line)); } catch { next.reject(new Error('Rust form fixture returned invalid JSON.')); } });
  child.on('error', error => { while (waiting.length) waiting.shift().reject(error); });
  child.on('close', code => { closed = true; while (waiting.length) waiting.shift().reject(new Error(`Rust form fixture exited ${code}.`)); });
  const query = command => new Promise((resolve, reject) => {
    if (closed) { reject(new Error('Rust form fixture is closed.')); return; }
    waiting.push({ resolve, reject }); child.stdin.write(JSON.stringify(command) + '\n');
  });
  const requests = [];
  const server = createServer(async (request, response) => {
    try {
      if (request.method === 'GET' && request.url === '/catalog') {
        response.setHeader('Content-Type', 'application/json'); response.end(JSON.stringify(await query({ kind: 'catalog' }))); return;
      }
      if (request.method !== 'POST' || request.url !== '/form') { response.writeHead(404); response.end(); return; }
      let bytes = 0; const chunks = [];
      for await (const chunk of request) { bytes += chunk.length; if (bytes > 65_536) { response.writeHead(413); response.end(); return; } chunks.push(chunk); }
      const value = JSON.parse(Buffer.concat(chunks).toString('utf8'));
      requests.push(structuredClone(value));
      const result = await query({ kind: 'form', request: value });
      response.setHeader('Content-Type', 'application/json'); response.end(JSON.stringify(result));
    } catch { response.writeHead(500); response.end(JSON.stringify({ ok: false, code: 'fixture_transport_failed' })); }
  });
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', resolve); });
  const origin = `http://127.0.0.1:${server.address().port}`;
  return {
    origin, requests, query, binary, binarySha256,
    async close() { await new Promise(resolve => server.close(resolve)); child.stdin.end(); await new Promise(resolve => { if (closed) resolve(); else child.once('close', resolve); }); reader.close(); if (sha(await fs.readFile(binary)) !== binarySha256) throw new Error('Rust fixture binary changed during verification.'); await fs.writeFile(path.join(output, 'rust-fixture-execution.json'), JSON.stringify({ binary, binarySha256, exitCode: child.exitCode, requests: requests.length, stderr: fixtureError, actualPolicy: 'Rust builtin_registry.project_form', credentialAccess: false, profileAccess: false, providerRequests: false }, null, 2) + '\n'); },
  };
}
