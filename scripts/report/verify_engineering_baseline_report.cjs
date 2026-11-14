// Offline artifact QA only. Never starts the desktop client or a game.
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const {pathToFileURL} = require('node:url');
const root = path.resolve(__dirname, '../..');
const {chromium} = require(path.join(root, '.build/report-browser/node_modules/playwright-core'));
const report = path.join(root, 'reports/first-product-engineering-20261018');
const out = path.join(root, '.build/database-architecture-report-qa');
const metadata = JSON.parse(fs.readFileSync(path.join(report, 'references/docs/architecture/database/schema-metadata.json')));
const manifest = JSON.parse(fs.readFileSync(path.join(report, 'report-manifest.json')));
(async () => {
  fs.mkdirSync(out, {recursive: true});
  const browser = await chromium.launch({executablePath: process.env.REPORT_CHROME_PATH || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless:true, args:['--disable-background-networking']});
  const errors = [], http = [], captures = [];
  const context = await browser.newContext({offline:true, viewport:{width:1440,height:1100}, reducedMotion:'reduce'});
  const page = await context.newPage();
  page.on('pageerror', e => errors.push(e.message));
  page.on('console', m => {if(m.type()==='error') errors.push(m.text());});
  page.on('request', r => {if(/^https?:/.test(r.url())) http.push(r.url());});
  async function capture(name, locator) {
    await (locator || page).screenshot({path:path.join(out,name)});
    captures.push(name);
  }
  async function noOverflow() {
    assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth <= innerWidth+1), 'page overflow');
  }
  try {
    await page.goto(pathToFileURL(path.join(report,'index.html')).href);
    await page.evaluate(()=>document.fonts.ready);
    assert.equal(await page.locator('h1').count(),1);
    assert.equal(await page.locator('#modules-table tbody tr').count(),16);
    assert.equal(await page.locator('#components-table tbody tr').count(),34);
    assert.equal(await page.locator('#test-matrix-table tbody tr').count(),128);
    assert.equal(await page.locator('#database-fields-table tbody tr').count(),metadata.counts.fields);
    assert.equal(await page.locator('#database-tables-table tbody tr').count(),metadata.counts.tables);
    assert.equal(manifest.database_fields, metadata.counts.fields);
    assert.equal(manifest.database_tables, metadata.counts.tables);
    assert.equal(manifest.runtime_tests, 'NOT_RUN');
    assert.match(await page.locator('#database-relationships').innerText(),/候选字段设计/);
    assert.match(await page.locator('#database-proof').innerText(),/未安装迁移/);
    assert.equal(await page.locator('#test-operations').count(),1);
    assert.equal(await page.locator('#gate-results tbody tr').count(),5);
    const boundaryRegistry = JSON.parse(fs.readFileSync(path.join(report,'references/docs/architecture/database/field-boundaries.json')));
    const storageRegistry = JSON.parse(fs.readFileSync(path.join(report,'references/docs/development/storage-acceptance.json')));
    assert.equal(await page.locator('#storage-acceptance-table tbody tr').count(),storageRegistry.checks.length);
    assert.match(await page.locator('#storage-enforcement').innerText(),/产品存储验收：blocked/);
    assert.ok((await page.locator('#storage-enforcement').innerText()).includes(String(boundaryRegistry.counts.probes)));
    assert.equal(manifest.storage_enforcement.boundary_probes,boundaryRegistry.counts.probes);
    await page.locator('#storage-enforcement h2').scrollIntoViewIfNeeded();
    await capture('08-storage-enforcement-desktop.png');
    await page.evaluate(()=>scrollTo(0,0));
    await noOverflow();
    await capture('01-overview-desktop.png');
    await page.locator('#database-relationships').scrollIntoViewIfNeeded();
    const graph = page.locator('#database-relationships img');
    await graph.evaluate(img=>img.decode());
    assert.ok(await graph.evaluate(img=>img.naturalWidth>0));
    await capture('02-database-relationships.png',page.locator('#database-relationships'));
    await page.locator('#database-fields input').fill('model_parameters');
    const expected = metadata.databases.flatMap(db=>db.tables).find(table=>table.name==='model_parameters').fields.length;
    assert.equal(await page.locator('#database-fields-table tbody tr:visible').count(),expected);
    assert.match(await page.locator('#database-fields-count').innerText(),new RegExp('^'+expected+' / '));
    await page.locator('#database-fields').scrollIntoViewIfNeeded();
    await capture('03-field-search-desktop.png');
    await page.locator('#database-fields input').fill('no-such-schema-field-qa');
    assert.equal(await page.locator('#database-fields-table tbody tr:visible').count(),0);
    await page.locator('#database-fields input').fill('');
    assert.equal(await page.locator('#database-fields-table tbody tr:visible').count(),metadata.counts.fields);
    const localLinks = await page.locator('a[href]').evaluateAll(nodes=>nodes.map(node=>node.getAttribute('href')).filter(h=>h&&!h.startsWith('#')&&!/^[a-z]+:/i.test(h)));
    for (const href of localLinks) {
      const target = path.resolve(report,decodeURIComponent(href.split('#')[0].split('?')[0]));
      assert.ok(fs.existsSync(target),'Missing report target: '+href);
    }
    await page.setViewportSize({width:390,height:844});
    await page.evaluate(()=>scrollTo(0,0));
    await noOverflow();
    await capture('04-overview-mobile.png');
    await page.locator('#database-relationships > .section-title').scrollIntoViewIfNeeded();
    const viewport = page.locator('#database-relationships .directory-viewport');
    assert.ok(await viewport.evaluate(el=>el.scrollWidth>el.clientWidth));
    await viewport.evaluate(el=>{el.scrollLeft=el.scrollWidth;});
    assert.ok(await viewport.evaluate(el=>el.scrollLeft>0));
    await noOverflow();
    await capture('05-database-mobile.png');
    await page.locator('#database-fields input').fill('connections');
    await page.locator('#database-fields').scrollIntoViewIfNeeded();
    await noOverflow();
    await capture('06-fields-mobile.png');
    await page.goto(pathToFileURL(path.join(report,'references/docs/architecture/database/relationships.svg')).href);
    await page.evaluate(()=>document.fonts.ready);
    const svgResult=await page.evaluate(()=>{
      const box=document.documentElement.viewBox.baseVal;
      return {width:box.width,height:box.height,rects:document.querySelectorAll('rect[id]').length,
        textOverflow:[...document.querySelectorAll('text')].filter(el=>{const b=el.getBBox();return b.x<0||b.y<0||b.x+b.width>box.width+1||b.y+b.height>box.height+1;}).map(el=>el.textContent)};
    });
    assert.equal(svgResult.rects,metadata.counts.tables);
    assert.deepEqual(svgResult.textOverflow,[]);
    await page.setViewportSize({width:1240,height:1100});
    await capture('07-database-svg-top.png');
    assert.deepEqual(errors,[]);
    assert.deepEqual(http,[]);
    const result={storage_acceptance_rows:storageRegistry.checks.length,field_boundary_probes:boundaryRegistry.counts.probes,production_storage_status:'blocked',artifact_qa_passed:true,scope:'offline architecture report only',database:metadata.counts,
      field_search:true,field_and_catalog_identity:true,local_links_checked:localLinks.length,
      desktop_and_narrow_screen:true,graph_loaded:true,svg_text_within_viewbox:true,
      console_errors:errors,http_requests:http,product_tests_executed:false,
      product_migrations_executed:false,baseline:manifest.baseline_id,captures};
    fs.writeFileSync(path.join(out,'verification.json'),JSON.stringify(result,null,2)+'\n');
    process.stdout.write(JSON.stringify(result)+'\n');
  } finally { await browser.close(); }
})().catch(error=>{console.error(error);process.exit(1);});
