/* Browser-only design QA helper. Exercise the authored popup, not hidden selectOption. */
const assert = require('node:assert/strict');
const cssValue = value => String(value).replace(/\\/g, '\\\\').replace(/"/g, '\\"');
async function chooseSelect(page, select, value) {
  const id = await select.getAttribute('id');
  assert.ok(id, 'Custom Select must have a stable backing identity');
  const trigger = page.locator('.luna-select-trigger[data-select-for="' + cssValue(id) + '"]');
  await trigger.waitFor({state:'visible'});
  await trigger.click();
  const panel = page.locator('.luna-select-panel[role="listbox"]');
  await panel.waitFor({state:'visible'});
  await panel.locator('[role="option"][data-value="' + cssValue(value) + '"]').click();
  assert.equal(await select.inputValue(), String(value), 'Selection updates the real authored field');
  assert.equal(await page.locator('.luna-select-panel').count(), 0, 'Selection closes the popup');
}
module.exports = {chooseSelect};
