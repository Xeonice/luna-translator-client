#!/usr/bin/env node
/** Loopback-only persistent Vite review server; never stops an unowned process. */
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn, spawnSync } from 'node:child_process';
import { openSync, closeSync } from 'node:fs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const directory = path.join(root, '.build/live-ui-preview');
const receipt = path.join(directory, 'server.json');
const origin = 'http://127.0.0.1:1421';
const options = process.argv.slice(2);
if (options.some(option => !['--status', '--stop'].includes(option)) || options.length > 1) throw Error('用法：node scripts/dev/live-ui-preview.mjs [--status|--stop]');
async function health() {
  try {
    const response = await fetch(origin + '/__luna_live_preview_health', { signal: AbortSignal.timeout(800), redirect: 'error' });
    if (!response.ok) throw Error('固定端口返回了其他服务。');
    const value = await response.json();
    if (value.kind !== 'luna-live-ui-preview-v1' || value.repository !== root || !Number.isInteger(value.pid)) throw Error('固定端口属于其他服务或工作区，拒绝接管。');
    return value;
  } catch (error) {
    if (error instanceof TypeError && error.cause?.code === 'ECONNREFUSED') return null;
    throw error;
  }
}
await fs.mkdir(directory, { recursive: true });
const existing = await health();
if (options[0] === '--status') { console.log(JSON.stringify({ running: Boolean(existing), url: origin + '/', pid: existing?.pid ?? null, data: 'controlled-memory-only' })); process.exit(0); }
if (options[0] === '--stop') {
  if (!existing) { console.log('预览服务当前未运行。'); process.exit(0); }
  const saved = JSON.parse(await fs.readFile(receipt, 'utf8'));
  if (saved.pid !== existing.pid || saved.repository !== root) throw Error('未找到匹配的本工作区启动回执，拒绝停止该进程。');
  const processIdentity = spawnSync('ps', ['-p', String(existing.pid), '-o', 'command='], { encoding: 'utf8' });
  if (processIdentity.status !== 0 || !processIdentity.stdout.includes(path.join(root, 'apps/desktop/node_modules/vite/bin/vite.js')) || !processIdentity.stdout.includes(path.join(root, 'apps/desktop/tests/live-preview/vite.config.ts'))) throw Error('当前进程与本工作区 Vite 命令身份不符，拒绝停止。');
  process.kill(existing.pid, 'SIGTERM');
  console.log(`已请求停止本工作区预览服务（PID ${existing.pid}）。`); process.exit(0);
}
if (existing) { console.log(`实时预览已在运行：${origin}/（PID ${existing.pid}）`); process.exit(0); }
const logfile = path.join(directory, 'server.log');
const descriptor = openSync(logfile, 'a');
const child = spawn(process.execPath, [path.join(root, 'apps/desktop/node_modules/vite/bin/vite.js'), '--config', path.join(root, 'apps/desktop/tests/live-preview/vite.config.ts')], {
  cwd: path.join(root, 'apps/desktop'), detached: true, stdio: ['ignore', descriptor, descriptor],
});
closeSync(descriptor);
child.unref();
let ready;
for (let attempt = 0; attempt < 100; attempt++) {
  try { ready = await health(); if (ready) break; } catch (error) { if (attempt === 99) throw error; }
  try { process.kill(child.pid, 0); } catch { throw Error(`预览服务启动失败；请查看 ${logfile}`); }
  await new Promise(resolve => setTimeout(resolve, 100));
}
if (!ready || ready.pid !== child.pid) throw Error(`固定端口未就绪或归属不符；请查看 ${logfile}。未终止任何其他服务。`);
await fs.writeFile(receipt, JSON.stringify({ kind: 'luna-live-ui-preview-server', repository: root, pid: ready.pid, url: origin + '/', logfile, startedAt: new Date().toISOString(), native: false, data: 'controlled-memory-only' }, null, 2) + '\n');
console.log(`实时界面预览：${origin}/\nPID ${ready.pid} · 保存实际客户端 TSX / CSS 后自动刷新\n样例数据；不访问个人资料或真实服务。\n日志：${logfile}`);
