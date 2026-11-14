import assert from 'node:assert/strict';

// Preconditions are read from real rendered React and the explicitly controlled
// projection. They identify a comparison state; they never manufacture expected
// pixels, hide elements or change application state.
export async function assertCaptureConditions(page, expected) {
  const actual = await page.evaluate(() => {
    const state = window.uiHarness?.state;
    const visible = n => !!(n.getClientRects().length && getComputedStyle(n).visibility !== 'hidden');
    const plain = n => { const clone = n.cloneNode(true); clone.querySelectorAll('rt,rp').forEach(x => x.remove()); return clone.textContent; };
    const source = [...document.querySelectorAll('[data-testid="source-text"]')].filter(visible);
    const menus = [...document.querySelectorAll('[role="menu"]')].filter(visible);
    const trigger = document.querySelector('[aria-label="更多阅读操作"]');
    return {
      windowRole: document.documentElement.dataset.windowRole,
      viewport: { width: innerWidth, height: innerHeight },
      theme: document.documentElement.dataset.theme,
      transparency: document.documentElement.dataset.transparency,
      visibleMenus: menus.length,
      menuTriggerExpanded: trigger?.getAttribute('aria-expanded') ?? null,
      sourceTexts: source.map(plain),
      sourceFonts: source.map(n => ({ size: getComputedStyle(n).fontSize, family: getComputedStyle(n).fontFamily })),
      current: state?.current ? { unitId: state.current.unitId, sourceText: state.current.sourceText, translations: state.current.translations.map(t => ({ text: t.correction?.text ?? t.text, providerLabel: t.providerLabel })) } : null,
      frozen: state?.learningObject ? { unitId: state.learningObject.unit.unitId, sourceText: state.learningObject.unit.sourceText, tokenIndex: state.learningObject.tokenIndex } : null,
      preferences: state?.preferences ? { sourceFontPt: state.preferences.sourceFontPt, translationFontPt: state.preferences.translationFontPt, segmentationEnabled: state.preferences.segmentationEnabled, showRuby: state.preferences.showRuby, showPos: state.preferences.showPos, readingDisplayMode: state.preferences.readingDisplayMode, hudBackgroundOpacity: state.preferences.hudBackgroundOpacity } : null,
      configuration: state?.configuration ? { draftHeadRevision: state.configuration.draftHeadRevision, appliedHeadRevision: state.configuration.appliedHeadRevision, draftEnabled: state.configuration.draft.bindings.map(b => ({ purpose: b.purpose, enabled: b.enabled })), appliedPresent: state.configuration.applied !== null } : null,
      sessionFollow: [...document.querySelectorAll('button[aria-pressed]')].find(n=>/^跟随新内容|^回到最新/.test(n.textContent.trim()))?.getAttribute('aria-pressed')==='true',
      sessionTab: document.querySelector('[aria-label="当前会话内容"] [aria-selected="true"]')?.textContent ?? null,
      selectedEntityTab: document.querySelector('[aria-label="模型配置实体"] [aria-pressed="true"]')?.textContent ?? null,
      settingsCategory: document.querySelector('[aria-label="设置分类"] [aria-current="page"]')?.textContent ?? null,
      dictionaryHeadwords: [...document.querySelectorAll('[aria-label^="词典词条 "]')].filter(visible).map(n => n.getAttribute('aria-label').slice(5)),
      hudOpacity: state?.preferences.hudBackgroundOpacity ?? null,
      hudFit: { naturalHeight: window.uiHarness?.hudFitRequests?.at(-1) ?? null, clampedHeight: window.uiHarness?.hudFitRequests?.length ? Math.max(140,Math.min(366,window.uiHarness.hudFitRequests.at(-1))) : null },
      hudControls: { segmentation: document.querySelector('[aria-label="可点击分词"]')?.getAttribute('aria-pressed') === 'true', ruby: document.querySelector('[aria-label="假名注音"]')?.getAttribute('aria-pressed') === 'true', pos: document.querySelector('[aria-label="词性提示"]')?.getAttribute('aria-pressed') === 'true', pin: document.querySelector('[aria-label="取消置顶"]')?.getAttribute('aria-pressed') === 'true' },
      gameCount: state?.games.length ?? null,
      session: state?.session ? { sourceKind: state.session.sourceKind, state: state.session.state, gameId: state.session.gameId } : null,
    };
  });
  assert.equal(actual.windowRole, expected.windowRole, 'Reference capture condition mismatch: window role');
  assert.deepEqual(actual.viewport, expected.viewport, 'Reference capture condition mismatch: viewport');
  for (const key of ['theme', 'transparency', 'sourceTexts', 'preferences', 'configuration', 'selectedEntityTab', 'settingsCategory', 'dictionaryHeadwords', 'gameCount', 'hudControls', 'sessionTab', 'hudOpacity', 'sessionFollow']) {
    if (Object.hasOwn(expected, key)) assert.deepEqual(actual[key], expected[key], `Reference capture condition mismatch: ${key}`);
  }
  for (const key of ['current', 'frozen', 'session']) if (Object.hasOwn(expected, key)) {
    if (expected[key] === null) assert.equal(actual[key], null, `Reference capture condition mismatch: ${key}`);
    else for (const [field, value] of Object.entries(expected[key])) assert.deepEqual(actual[key]?.[field], value, `Reference capture condition mismatch: ${key}.${field}`);
  }
  if (expected.hudFitHeight !== undefined) {
    assert.equal(actual.hudFit.clampedHeight,expected.hudFitHeight,'Natural HUD fit request must produce the independently declared native viewport height');
    assert.equal(actual.viewport.height,actual.hudFit.clampedHeight,'Fixed screenshot viewport cannot conceal a different native resize request');
  }
  if (expected.sourceFont) assert.ok(actual.sourceFonts.every(font => font.size === expected.sourceFont), 'Reference capture condition mismatch: rendered source font');
  if (expected.hudMenu === 'closed') {
    assert.equal(actual.visibleMenus, 0, 'Reference capture condition mismatch: HUD menu must be closed');
    assert.equal(actual.menuTriggerExpanded, 'false', 'Reference capture condition mismatch: HUD menu trigger must be collapsed');
  } else if (expected.hudMenu === 'open') {
    assert.equal(actual.visibleMenus, 1, 'Reference capture condition mismatch: HUD menu must be open');
    assert.equal(actual.menuTriggerExpanded, 'true', 'Reference capture condition mismatch: HUD menu trigger must be expanded');
  } else if (expected.hudMenu !== undefined) throw new Error('Unsupported reference capture menu state');
  const fields = [];
  for (const field of expected.fields ?? []) {
    const input = page.getByRole(field.role, { name: field.name, exact: field.exact ?? true });
    assert.equal(await input.count(), 1, `Reference capture field must be unique: ${field.name}`);
    const customSelect = field.role === 'combobox' && await input.getAttribute('data-select-value') !== null;
    const value = customSelect ? await input.getAttribute('data-select-value') : await input.inputValue();
    assert.equal(value, field.value, `Reference capture condition mismatch: field ${field.name}`);
    let options;
    if (field.options && customSelect) {
      await input.click();
      options = await page.getByRole('option').evaluateAll(nodes => nodes.map(node => ({ value: node.getAttribute('data-select-value'), label: node.textContent })));
      await page.keyboard.press('Escape');
      // React Aria restores focus after its overlay FocusScope unmounts. Observe
      // that actual keyboard contract before the neutral composition is restored.
      await page.waitForFunction(id => document.activeElement?.id === id, await input.getAttribute('id'));
      await input.evaluate(node => node.blur());
    } else if (field.options) options = await input.locator('option').evaluateAll(nodes => nodes.map(node => ({ value: node.value, label: node.textContent }))); 
    if (field.options) assert.deepEqual(options, field.options, `Reference capture condition mismatch: options ${field.name}`);
    fields.push({ role: field.role, name: field.name, value, ...(options ? {options} : {}) });
  }
  return { ...actual, hudFitHeight: actual.hudFit.clampedHeight, fields };
}
