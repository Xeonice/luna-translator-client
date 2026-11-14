/* Frontend IPC contract fixtures only. No native capture or Hook injection is performed. */
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const http = require('node:http');
const { chromium } = require('../../.build/report-browser/node_modules/playwright-core');
const repo = path.resolve(__dirname, '../..');
const dist = path.join(repo, 'apps/desktop/dist');
const output = path.join(repo, 'diagnostics/bind-inject-20261018/frontend-contract');
const checks = [];
const errors = [];
const shots = [];

(async () => {
  await fs.mkdir(output, { recursive: true });
  const server = http.createServer(async (req, res) => {
    try {
      const relative = decodeURIComponent(new URL(req.url, 'http://localhost').pathname);
      const file = path.join(dist, relative === '/' ? 'index.html' : relative);
      if (!file.startsWith(dist + path.sep)) { res.writeHead(403); res.end(); return; }
      const content = await fs.readFile(file);
      res.setHeader('Content-Type', file.endsWith('.js') ? 'text/javascript' : file.endsWith('.css') ? 'text/css' : 'text/html');
      res.end(content);
    } catch { res.writeHead(404); res.end(); }
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true });
  try {
    const page = await browser.newPage({ viewport: { width: 1240, height: 1080 }, deviceScaleFactor: 1 });
    page.on('pageerror', (error) => errors.push(error.message));
    await page.addInitScript(() => {
      const settings = { wine_path: '/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine', bottle: 'UI-contract-test' };
      const target = { pid: 120, exe: 'alpha.exe', full_image_path: 'C:\\TestGames\\alpha.exe', creation_filetime: '13370000001', architecture: 'x64', errors: [] };
      const windowA = { hwnd: '90210', title: 'ウィンドウ A（UI 契約テスト）', class_name: 'TestGameWindow', rect: { left: 0, top: 0, right: 1280, bottom: 720 }, target };
      const windowB = { ...windowA, hwnd: '90211', title: '<img src=x onerror=alert(1)> テスト B', target: { ...target, pid: 121, exe: 'beta.exe', full_image_path: 'C:\\TestGames\\beta.exe', creation_filetime: '13370000002' } };
      const client = { sequence: 1, session_id: null, status: 'idle', host_running: false, settings: null, target: null, streams: [], session_log_dir: null, errors: [], logs: [], integrity_lost: false, text_events: 0, shutdown_acknowledged: false, exit_code: null };
      window.__bindingTest = { calls: [], client, binding: { binding: null, status: 'unbound', last_error: null }, windows: [windowA, windowB], pick: { status: 'picked', window: windowB }, failList: false, failStatus: false, failStart: false, connectDelay: 0, activePoll: 0, maxPoll: 0, statusDelay: 0, starts: 0, sessionCounter: 0 };
      window.isTauri = true;
      window.__TAURI_INTERNALS__ = { invoke: async (command, args) => {
        const state = window.__bindingTest;
        state.calls.push({ command, args });
        if (command === 'default_settings') return settings;
        if (command === 'client_snapshot') return structuredClone(state.client);
        if (command === 'window_binding_status') {
          state.activePoll++; state.maxPoll = Math.max(state.maxPoll, state.activePoll);
          const captured = structuredClone(state.binding);
          if (state.statusDelay) await new Promise((resolve) => setTimeout(resolve, state.statusDelay));
          state.activePoll--;
          if (state.failStatus) throw new Error('契約テスト：状態照会失敗');
          return captured;
        }
        if (command === 'discover_windows') { if (state.failList) throw new Error('契約テスト：一覧取得失敗'); return structuredClone(state.windows); }
        if (command === 'pick_window') return structuredClone(state.pick);
        if (command === 'bind_and_connect_window') {
          if (state.connectDelay) await new Promise((resolve) => setTimeout(resolve, state.connectDelay));
          const same = JSON.stringify(state.client.target) === JSON.stringify(args.window.target) && JSON.stringify(state.client.settings) === JSON.stringify(args.settings);
          const result = (status, error = null) => structuredClone({ status, binding: state.binding, client: state.client, error });
          if (state.client.host_running && !same && !args.replaceExisting) return result('replace_required', '已有活动连接，请确认切换。');
          state.binding = { status: 'bound', binding: { window: args.window, settings: args.settings, bound_at_ms: Date.now() }, last_error: null };
          if (same && state.client.host_running) return result('reused');
          state.starts++;
          state.client = { ...client, sequence: state.client.sequence + 1, settings: args.settings, target: args.window.target, status: state.failStart ? 'error' : 'starting', host_running: !state.failStart, session_id: `fixture-${++state.sessionCounter}` };
          return result(state.failStart ? 'hook_failed' : 'connecting', state.failStart ? '契約テスト：Hook 组件启动失败' : null);
        }
        if (command === 'unbind_window') { state.binding = { status: 'unbound', binding: null, last_error: null }; return structuredClone(state.binding); }
        throw new Error(`Unexpected contract command: ${command}`);
      } };
      document.addEventListener('DOMContentLoaded', () => {
        const banner = document.createElement('div');
        banner.textContent = 'FIXTURE · UI 合同测试（非原生窗口 / 非真实 Hook）';
        banner.style.cssText = 'position:fixed;bottom:8px;right:12px;font:11px system-ui;color:#c4c5cf;background:#232631;padding:6px 10px;border:1px solid #555665;border-radius:7px;z-index:2147483647;pointer-events:none';
        document.body.append(banner);
      });
    });
    await page.goto(`http://127.0.0.1:${server.address().port}`);
    const panel = page.getByRole('region', { name: '游戏窗口关联' });
    const dialog = page.getByRole('dialog');
    const currentText = page.getByTestId('current-text');
    async function capture(name) { const file = `${name}.png`; await page.screenshot({ path: path.join(output, file), fullPage: true }); shots.push(file); }
    async function hasText(locator, text) { await locator.filter({ hasText: text }).waitFor(); }
    async function commandCount(command) { return page.evaluate((name) => window.__bindingTest.calls.filter((item) => item.command === name).length, command); }
    async function publishText(text) {
      await page.evaluate((text) => {
        const state = window.__bindingTest;
        const line = { sequence: 1, timestamp_ms: 1792320000000, text };
        state.client = { ...state.client, sequence: state.client.sequence + 1, status: 'connected', host_running: true, text_events: 1, streams: [{ key: `fixture-stream-${state.client.target.pid}`, id: { pid: state.client.target.pid, address: '1', context: '2', context2: '3' }, hook_name: 'FixtureHook', hook_code: 'FIXTURE', active: true, latest_text: text, history: [line], text_events: 1 }] };
      }, text);
      await hasText(currentText, text);
    }
    async function choose(index = 0) {
      await panel.getByRole('button', { name: /^(选择窗口|替换窗口)…$/ }).click();
      await page.locator('.window-candidate').nth(index).click();
    }
    await hasText(panel, '尚未关联窗口');
    assert.equal(await page.locator('.manual-connection').getAttribute('open'), null);
    await capture('01-empty-binding-contract');
    await choose();
    await hasText(page.locator('.window-review'), 'C:\\TestGames\\alpha.exe');
    await capture('02-associate-connect-review-contract');
    await page.evaluate(() => { window.__bindingTest.connectDelay = 200; });
    await dialog.getByRole('button', { name: '关联并接入', exact: true }).click();
    assert.equal(await dialog.getByRole('button', { name: '正在关联并接入…', exact: true }).isEnabled(), false);
    await dialog.waitFor({ state: 'hidden' });
    await hasText(panel, '正在接入游戏');
    assert.equal(await commandCount('bind_and_connect_window'), 1);
    assert.equal(await currentText.count(), 0);
    checks.push('关联确认只调用一次新 IPC 自动接入；忙时禁用重复提交，未伪造原文');

    await page.evaluate(() => { const s = window.__bindingTest; s.client = { ...s.client, sequence: s.client.sequence + 1, status: 'connected' }; });
    await hasText(panel, '已接入，等待文本');
    await capture('03-connected-waiting-text-contract');
    const original = 'これは UI 契約テスト用の原文です。';
    await publishText(original);
    await hasText(panel, '正在接收原文');
    await capture('04-receiving-fixture-text-contract');
    checks.push('启动、已接入待文本、收到原文分阶段；真实 fixture 流才显示候选与历史');

    await choose();
    await hasText(page.locator('.window-review'), '将复用当前连接');
    await dialog.getByRole('button', { name: '关联并接入', exact: true }).click();
    await dialog.waitFor({ state: 'hidden' });
    assert.equal(await page.evaluate(() => window.__bindingTest.starts), 1);
    assert.equal(await currentText.textContent(), original);
    assert.equal(await page.locator('.history-list li').count(), 1);
    checks.push('相同活动目标复用连接，未重复启动注入且保留当前流与历史');

    const beforeCancel = await commandCount('bind_and_connect_window');
    await choose(1);
    await hasText(page.locator('.window-switch-review'), '当前：alpha.exe');
    await hasText(page.locator('.window-switch-review'), '切换到：beta.exe');
    assert.equal(await page.locator('.window-dialog img').count(), 0);
    await capture('05-switch-explicit-review-contract');
    await page.keyboard.press('Escape');
    await dialog.waitFor({ state: 'hidden' });
    assert.equal(await commandCount('bind_and_connect_window'), beforeCancel);
    assert.equal(await currentText.textContent(), original);
    await choose(1);
    await dialog.getByRole('button', { name: '切换并接入', exact: true }).click();
    await dialog.waitFor({ state: 'hidden' });
    assert.equal(await page.evaluate(() => window.__bindingTest.calls.filter((c) => c.command === 'bind_and_connect_window').at(-1).args.replaceExisting), true);
    assert.equal(await page.evaluate(() => window.__bindingTest.client.target.exe), 'beta.exe');
    assert.equal(await page.locator('select[aria-label="游戏进程"]').inputValue(), '121|13370000002|C:\\TestGames\\beta.exe');
    checks.push('异目标显示旧→新及断开影响，仅切换确认传 true；Esc 无后台动作，标题转义与目标同步');

    await page.evaluate(() => { window.__bindingTest.failStart = true; });
    await choose(0);
    await dialog.getByRole('button', { name: '切换并接入', exact: true }).click();
    await dialog.waitFor({ state: 'hidden' });
    await hasText(panel, '窗口已关联，接入失败');
    await hasText(panel, 'Hook 组件启动失败');
    const discoveryBeforeRetry = await commandCount('discover_windows');
    await capture('06-hook-failure-retry-contract');
    await page.evaluate(() => { window.__bindingTest.failStart = false; });
    await panel.getByRole('button', { name: '重试接入', exact: true }).click();
    await page.waitForFunction(() => window.__bindingTest.starts === 4);
    await hasText(panel, '正在接入游戏');
    assert.equal(await commandCount('discover_windows'), discoveryBeforeRetry);
    assert.equal(await page.evaluate(() => window.__bindingTest.calls.filter((c) => c.command === 'bind_and_connect_window').at(-1).args.window.target.exe), 'alpha.exe');
    checks.push('Hook 失败保留窗口关联；重试同一核验窗口，无需重新选窗口或手动进程连接');

    await publishText(original);
    await panel.getByRole('button', { name: '解除关联', exact: true }).click();
    await hasText(panel, '尚未关联窗口');
    assert.equal(await currentText.textContent(), original);
    assert.equal(await page.locator('.history-list li').count(), 1);
    await page.evaluate(() => { window.__bindingTest.pick = { status: 'cancelled', window: null }; });
    const beforePickCancel = await commandCount('bind_and_connect_window');
    await panel.getByRole('button', { name: '点击游戏窗口…', exact: true }).click();
    await hasText(dialog, '已取消点选');
    assert.equal(await dialog.getByRole('button', { name: '关联并接入', exact: true }).isEnabled(), false);
    await dialog.getByRole('button', { name: '取消', exact: true }).click();
    assert.equal(await commandCount('bind_and_connect_window'), beforePickCancel);
    await capture('07-unbound-retains-text-contract');
    checks.push('解除窗口仅清关联而保留取文/当前原文/历史；取消点选不注入');

    await page.evaluate(() => { window.__bindingTest.binding = { status: 'window_closed', binding: null, last_error: null }; });
    await hasText(panel, '原窗口已关闭，关联已解除');
    assert.equal(await page.locator('.binding-indicator.is-bound').count(), 0);
    await page.evaluate(() => { window.__bindingTest.failList = true; });
    await panel.getByRole('button', { name: '选择窗口…', exact: true }).click();
    await hasText(dialog, '一覧取得失敗');
    await dialog.getByRole('button', { name: '取消', exact: true }).click();
    assert.equal(await currentText.textContent(), original);
    checks.push('窗口关闭或列表故障不冒充成功，不影响已有真实流与原文');

    await page.evaluate(() => { window.__bindingTest.statusDelay = 1100; });
    await page.waitForTimeout(2300);
    const counters = await page.evaluate(() => ({ maxPoll: window.__bindingTest.maxPoll, starts: window.__bindingTest.starts, windowConnectCalls: window.__bindingTest.calls.filter(({ command }) => command === 'bind_and_connect_window').length, oldBindCalls: window.__bindingTest.calls.filter(({ command }) => command === 'bind_window').length, directHookCalls: window.__bindingTest.calls.filter(({ command }) => command === 'connect_target' || command === 'disconnect_host').length }));
    assert.equal(counters.maxPoll, 1);
    assert.equal(counters.starts, 4);
    assert.equal(counters.oldBindCalls, 0);
    assert.equal(counters.directHookCalls, 0);
    assert.deepEqual(errors, []);
    checks.push('慢状态响应不重叠；窗口流程仅用新 IPC，未调用旧 bind 或额外 connect/disconnect');
    await fs.writeFile(path.join(output, 'qa.json'), JSON.stringify({ kind: 'frontend-ipc-contract-only', native_window_capture_verified: false, native_hook_injection_verified: false, passed: checks.length, checks, screenshots: shots, console_errors: errors, ...counters }, null, 2));
    console.log(JSON.stringify({ kind: 'frontend-ipc-contract-only', passed: checks.length, screenshots: shots.length, console_errors: errors, ...counters }));
  } finally { await browser.close(); await new Promise((resolve) => server.close(resolve)); }
})().catch((error) => { console.error(error); process.exitCode = 1; });
