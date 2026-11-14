/* Offline artifact QA only. Does not execute GL product/runtime regression. */
const assert = require('node:assert/strict');
const { chooseSelect } = require('./design_controls.cjs');

/**
 * Extend the first-product browser QA using its existing offline page/helpers.
 * Usage: await require('./verify_game_onboarding_cases.cjs').run({page,open,state,capture,checks});
 * Captures 38–47; no source mutation, real process discovery, or service requests.
 */
async function run({ page, open, state, capture, checks }) {
  const glAction = (action, suffix = '') => page.locator(`#app-dialog [data-gl-action="${action}"]${suffix}`);
  const glField = name => page.locator(`#app-dialog [data-gl-field="${name}"]`);
  const confirm = () => page.locator('#dialog-confirm').click();
  const cancel = () => page.locator('#dialog-cancel').click();
  const snapshot = () => state(() => ({
    session: {
      gameId: LunaDesign.state.session.gameId,
      status: LunaDesign.state.session.status,
      generation: LunaDesign.state.session.generation,
      streams: [...LunaDesign.state.session.selectedStreams],
      sessionId: LunaDesign.state.session.sessionId
    },
    current: LunaDesign.state.current && {
      id: LunaDesign.state.current.id,
      source: LunaDesign.state.current.source,
      language: LunaDesign.state.current.language,
      translation: LunaDesign.state.current.translation,
      translationStatus: LunaDesign.state.current.translationStatus
    },
    history: LunaDesign.state.history.map(record => ({ id: record.id, source: record.source, translation: record.translation }))
  }));
  const showMain = async pageName => {
    await page.locator('#menu-window').click();
    await page.locator('[data-menu-action="open-main"]').click();
    if (pageName) await page.locator(`#main-window .main-sidebar [data-page="${pageName}"]`).click();
  };
  const addFixture = async gameId => {
    await page.locator('#library-add').click();
    await glAction('show-fixtures').click();
    await glAction('add-fixture', `[data-gl-id="${gameId}"]`).click();
    await confirm();
  };
  const connectSheet = async gameId => {
    await page.waitForFunction(() => LunaDesign.state.sheet?.step === 3);
    assert.equal(await page.locator('#sheet-next').isDisabled(), true);
    await page.locator(`[data-stream="${gameId}-body"]`).check();
    await page.locator('#sheet-next').click();
    await page.waitForFunction(() => !!LunaDesign.state.current);
  };

  await open('first-run');
  assert.equal(await state(() => LunaLibrary.libraryGames().length), 0);
  assert.equal(await state(() => LunaDesign.state.selectedGame), null);
  assert.equal(await state(() => LunaDesign.state.current), null);
  assert.equal(await state(() => LunaDesign.state.session.status), 'idle');
  assert.equal(await page.locator('.game-card').count(), 0);
  assert.equal(await page.locator('#hud-window').isVisible(), true);
  assert.match(await page.locator('#hud-source').innerText(), /等待输入/);
  assert.equal(await state(() => LunaSettings.effective(LunaDesign.state.settings, 'translation', null).enabled), false);
  await capture('38-library-empty.png', '首次打开 · 真实空库与等待输入', '#desktop', 'onboarding');

  await page.locator('#library-add').click();
  assert.equal(await page.locator('#dialog-confirm').isDisabled(), true);
  await glAction('show-fixtures').click();
  assert.equal(await glAction('add-fixture').count(), 4);
  await capture('39-add-game-entry.png', '添加游戏 · 文件选择与明确标记的示例程序', '#app-dialog', 'onboarding');
  await glAction('add-fixture', '[data-gl-id="sakura"]').click();
  assert.equal(await glField('environment').inputValue(), 'crossover');
  assert.equal(await glField('bottle').inputValue(), 'Sakura-no-Uta-10th');
  assert.match(await glField('targetPath').inputValue(), /sakuranouta10th\.exe$/i);
  assert.equal(await glField('arch').inputValue(), 'x64');
  await capture('40-add-game-review.png', '核对游戏资料 · 启动程序与取文目标分别保存', '#app-dialog', 'onboarding');
  await confirm();
  assert.equal(await state(() => LunaLibrary.libraryGames().length), 1);
  assert.equal(await state(() => LunaDesign.state.current), null);
  assert.equal(await state(() => LunaDesign.state.session.status), 'idle');
  assert.equal(await state(() => LunaLibrary.getBinding(LunaDesign.state.selectedGame)), null);
  await addFixture('sakura');
  assert.equal(await state(() => LunaLibrary.libraryGames().length), 1, 'A duplicate complete target reuses its library entry');
  checks.push('onboarding_empty_library_adds_metadata_without_connecting_or_binding_and_reuses_duplicate_target');

  // Selecting the app only previews a rejected target; it cannot attach.
  await page.locator('#library-running').click();
  await glAction('select-target', '[data-gl-id="luna-self"]').click();
  assert.equal(await page.locator('#dialog-confirm').isDisabled(), true);
  assert.match(await page.locator('#app-dialog').innerText(), /不能接入 Luna 自身/);
  assert.equal(await state(() => LunaDesign.state.session.status), 'idle');
  await cancel();

  await page.locator('#library-running').click();
  await glAction('pick-window').click();
  assert.equal(await page.locator('#app-dialog .gl-pick-grid .gl-target img').count(), 4);
  const ownWindowRow = await glAction('select-target', '[data-gl-id="luna-self"]').boundingBox();
  assert.ok(ownWindowRow.width > ownWindowRow.height * 3, 'The app safety choice stays a readable horizontal row');
  await capture('41-window-picker.png', '从运行窗口开始 · 点击目标缩略图', '#app-dialog', 'onboarding');
  await glAction('select-target', '[data-gl-id="sample-window-sakura"]').click();
  const mapped = await state(() => LunaLibrary.windowChoices().find(choice => choice.id === 'sample-window-sakura'));
  assert.notEqual(mapped.hostPid, mapped.windowsPid);
  assert.equal(mapped.environment, 'crossover');
  assert.equal(mapped.bottle, 'Sakura-no-Uta-10th');
  assert.match(await page.locator('#app-dialog .gl-mapping').innerText(), /Windows 游戏进程[\s\S]*环境与容器/);
  await page.locator('#app-dialog').evaluate(node => { node.scrollTop = node.scrollHeight; });
  await capture('42-window-mapping.png', '确认窗口映射 · 宿主、Windows 进程、容器与完整路径', '#app-dialog', 'onboarding');
  await confirm();
  assert.equal(await page.locator('#connection-sheet').isVisible(), true);
  assert.equal(await state(() => LunaDesign.state.connection.status), 'attaching');
  assert.equal(await state(() => LunaDesign.state.current), null);
  assert.equal(await state(() => !!LunaLibrary.getBinding('sakura')), true);
  await connectSheet('sakura');
  assert.equal(await state(() => LunaDesign.state.session.status), 'connected');
  assert.equal(await state(() => LunaDesign.state.current.source), await state(() => LunaDesignData.games.find(game => game.id === 'sakura').lines[0].source));
  assert.equal(await state(() => LunaDesign.state.current.translationStatus), 'disabled');
  await capture('43-window-connected-reading.png', '关联确认自动接入 · 选择正文后原文先显示', '#desktop', 'onboarding');
  checks.push('onboarding_window_picker_separates_host_and_windows_identities_and_requires_hook_stream_confirmation');

  await showMain('session');
  const stableReading = await snapshot();
  await page.locator('#main-content .session-controls [data-action="unbind-window"]').click();
  assert.equal(await state(() => LunaLibrary.getBinding(LunaDesign.state.session.gameId)), null);
  assert.deepEqual(await snapshot(), stableReading);
  await page.locator('#main-content .session-controls [data-action="bind-window"]').click();
  await glAction('select-target', '[data-gl-id="luna-self"]').click();
  assert.equal(await page.locator('#dialog-confirm').isDisabled(), true);
  assert.deepEqual(await snapshot(), stableReading);
  await glAction('select-target', '[data-gl-id="sample-window-mo"]').click();
  assert.match(await page.locator('#app-dialog .gl-switch-review').innerText(), /将切换取文目标/);
  assert.match(await page.locator('#dialog-confirm').innerText(), /切换并接入/);
  const switchFooter = await page.locator('#dialog-confirm').boundingBox();
  assert.ok(switchFooter && switchFooter.y + switchFooter.height <= 1100, 'Switch confirmation must stay visible when review content is long');
  await page.locator('#app-dialog .dialog-body').evaluate(node => { node.scrollTop = node.scrollHeight; });
  await capture('48-window-switch-confirm.png', '切换游戏目标 · 确认前原会话与历史保留', '#app-dialog', 'window-binding');
  await cancel();
  assert.equal(await state(() => LunaLibrary.getBinding('sakura')), null);
  assert.deepEqual(await snapshot(), stableReading);
  await page.locator('#main-content .session-controls [data-action="bind-window"]').click();
  await glAction('select-target', '[data-gl-id="sample-window-sakura"]').click();
  await page.locator('#app-dialog').evaluate(node => { node.scrollTop = node.scrollHeight; });
  await capture('44-window-binding.png', '关联同一运行目标 · 复用取文且不重复注入', '#app-dialog', 'window-binding');
  await confirm();
  const binding = await state(() => LunaLibrary.getBinding('sakura'));
  assert.equal(binding.recordId, 'sakura');
  assert.equal(binding.bottle, 'Sakura-no-Uta-10th');
  assert.deepEqual(await snapshot(), stableReading);
  await page.locator('#main-content .session-controls [data-action="unbind-window"]').click();
  assert.equal(await state(() => LunaLibrary.getBinding('sakura')), null);
  assert.deepEqual(await snapshot(), stableReading);
  await capture('45-window-unbound.png', '解除窗口关联 · 当前取文与历史继续', '#main-window', 'window-binding');
  assert.equal(await page.locator('#main-content .session-controls [data-action="unbind-window"]').count(), 0);
  assert.match(await page.locator('#main-content .session-controls [data-action="bind-window"]').innerText(), /关联并接入游戏窗口/);
  assert.match(await page.locator('#main-content').innerText(), /翻译用途尚未启用/);
  checks.push('single_confirmation_starts_hook_reuse_keeps_session_switch_requires_review_cancel_preserves_and_unbind_keeps_reading');

  // An active game's library metadata may change; its already-connected target and
  // source-language snapshot must remain fixed until an explicit reconnect.
  await showMain('library');
  const srIdentity = await state(() => LunaLibrary.allGames().find(game => game.id === 'sr'));
  await page.locator('[data-action="game-edit"]').click();
  await chooseSelect(page, glField('environment'), srIdentity.environment);
  await glField('bottle').fill(srIdentity.bottle);
  await glField('launchPath').fill(srIdentity.launchPath);
  await glField('targetPath').fill(srIdentity.targetPath);
  await chooseSelect(page, glField('arch'), srIdentity.arch);
  await confirm();
  assert.deepEqual(await snapshot(), stableReading);
  assert.equal(await state(() => LunaLibrary.libraryGames()[0].fixtureSampleId), 'sr');
  assert.equal(await state(() => LunaDesign.state.session.gameSnapshot.language), 'ja');
  await page.locator('#main-open-settings').click();
  await page.locator('#settings-content [data-settings-action="purpose"][data-value="translation"]').click();
  await page.locator('#settings-content [data-settings-field="binding.enabled"]').check();
  await page.locator('#settings-content [data-settings-action="apply"]').click();
  await confirm();
  await page.locator('[data-action="close-settings"]').click();
  await page.locator('#review-next').click();
  await page.waitForFunction(() => LunaDesign.state.current?.translationStatus === 'complete');
  assert.equal(await state(() => LunaDesign.state.current.language), 'ja');
  assert.equal(await state(() => LunaDesign.state.current.source), await state(() => LunaDesignData.games.find(game => game.id === 'sakura').lines[1].source));
  assert.equal(await state(() => LunaDesign.state.current.translation), await state(() => LunaDesignData.games.find(game => game.id === 'sakura').lines[1].translation));
  const afterAdvance = await snapshot();
  await showMain('session');
  await page.locator('#main-content .session-controls [data-action="bind-window"]').click();
  await glAction('select-target', '[data-gl-id="sample-window-sr"]').click();
  assert.match(await page.locator('#app-dialog .gl-switch-review').innerText(), /将切换取文目标/);
  assert.deepEqual(await snapshot(), afterAdvance);
  await glAction('select-target', '[data-gl-id="sample-window-sakura"]').click();
  await confirm();
  assert.deepEqual(await snapshot(), afterAdvance, 'Window rebinding follows the frozen active target, not edited library metadata');
  await page.locator('#main-window .main-sidebar [data-page="library"]').click();
  await page.locator('[data-action="game-edit"]').click();
  await glField('title').fill('资料重命名，不改变当前会话');
  await confirm();
  assert.deepEqual(await snapshot(), afterAdvance);
  await page.locator('[data-action="game-remove"]').click();
  await confirm();
  assert.equal(await state(() => LunaLibrary.libraryGames().length), 0);
  assert.deepEqual(await snapshot(), afterAdvance);
  await page.locator('#main-window .main-sidebar [data-page="session"]').click();
  assert.match(await page.locator('#main-content').innerText(), /已连接/);
  await page.locator('#main-window .main-sidebar [data-page="history"]').click();
  assert.equal(await page.locator('.history-record').count(), afterAdvance.history.length);
  await page.locator('#main-window .main-sidebar [data-page="library"]').click();
  await page.locator('#library-add').click();
  await glAction('show-fixtures').click();
  await glAction('add-fixture', '[data-gl-id="never7"]').click();
  await cancel();
  assert.equal(await state(() => LunaLibrary.libraryGames().length), 0);
  assert.deepEqual(await snapshot(), afterAdvance);
  checks.push('editing_active_library_identity_preserves_source_language_snapshot_advance_and_translation_and_removal_keeps_session_history');

  // The same full identity cannot overwrite another existing owned record.
  await open('existing-library');
  await page.locator('[data-action="game-edit"]').click();
  const existingSR = await state(() => LunaLibrary.libraryGames().find(game => game.id === 'sr'));
  await chooseSelect(page, glField('environment'), existingSR.environment);
  await glField('bottle').fill(existingSR.bottle);
  await glField('launchPath').fill(existingSR.launchPath);
  await glField('targetPath').fill(existingSR.targetPath);
  await chooseSelect(page, glField('arch'), existingSR.arch);
  await confirm();
  assert.equal(await page.locator('#app-dialog').isVisible(), true);
  assert.match(await page.locator('#app-dialog .gl-errors').innerText(), /已属于/);
  assert.equal(await state(() => LunaLibrary.libraryGames().length), 4);
  assert.match(await state(() => LunaLibrary.libraryGames().find(game => game.id === 'sakura').exe), /sakuranouta10th/i);
  await cancel();
  checks.push('library_edit_rejects_overwriting_another_owned_complete_target_identity');

  await open('first-run');
  await page.locator('#library-add').click();
  await page.locator('#gl-game-files').setInputFiles([
    { name: 'Game.exe', mimeType: 'application/octet-stream', buffer: Buffer.alloc(0) },
    { name: 'Game.exe', mimeType: 'application/octet-stream', buffer: Buffer.alloc(0) }
  ]);
  assert.equal(await page.locator('#app-dialog [data-gl-action="select-item"]').count(), 2);
  await confirm();
  assert.equal(await state(() => LunaLibrary.libraryGames().length), 0);
  assert.equal(await page.locator('#app-dialog').isVisible(), true);
  assert.match(await page.locator('#app-dialog .gl-errors').innerText(), /第 1 项/);
  for (const [index, bottle] of ['Unknown-Bottle-A', 'Unknown-Bottle-B'].entries()) {
    await glAction('select-item', `[data-gl-index="${index}"]`).click();
    await glField('title').fill(`同名程序 · ${index + 1}`);
    await glField('bottle').fill(bottle);
    await glField('launchPath').fill('C:\\Games\\Unknown\\Game.exe');
    await glField('targetPath').fill('C:\\Games\\Unknown\\Game.exe');
    await chooseSelect(page, glField('arch'), 'x86');
    await glField('cwd').fill('C:\\Games\\Unknown');
  }
  await glAction('select-item', '[data-gl-index="0"]').click();
  assert.match(await page.locator('#app-dialog .gl-import-items').innerText(), /Unknown-Bottle-A[\s\S]*Unknown-Bottle-B/);
  await capture('46-custom-batch-review.png', '两个同名程序 · 必须补齐路径与不同容器', '#app-dialog', 'onboarding');
  await confirm();
  const unknowns = await state(() => LunaLibrary.libraryGames());
  assert.equal(unknowns.length, 2);
  assert.equal(new Set(unknowns.map(game => game.id)).size, 2);
  assert.equal(new Set(unknowns.map(game => game.bottle)).size, 2);
  assert.equal(unknowns.every(game => !game.lines.length && !game.streams.length && game.scene === null && game.fixtureSampleId === null), true);
  assert.equal(await state(() => LunaDesign.state.current), null);
  await page.locator('#library-running').click();
  assert.equal(await page.locator('#dialog-confirm').isDisabled(), true);
  assert.match(await page.locator('#app-dialog').innerText(), /选择窗口或进程/);
  assert.equal(await state(() => LunaDesign.state.session.status), 'idle');
  assert.equal(await state(() => LunaDesign.state.current), null);
  assert.equal(await state(() => LunaDesign.state.history.length), 0);
  await capture('47-unknown-target-check.png', '未知程序 · 先选择实际运行目标，不借用其他正文', '#app-dialog', 'onboarding');
  checks.push('two_filename_only_files_require_identity_fields_and_different_bottles_remain_distinct_without_fixture_text_or_fake_connection');
}

module.exports = { run };
