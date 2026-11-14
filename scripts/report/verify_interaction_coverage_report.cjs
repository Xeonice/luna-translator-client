// Offline report rendering QA. This never starts the product or a game.
'use strict';
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict'),crypto=require('node:crypto');
const {pathToFileURL}=require('node:url');
const {chromium}=require(path.resolve('node_modules/playwright-core'));
const report=path.resolve(process.argv[2]),out=path.resolve(process.argv[3]||'.build/coverage-architecture-20261019/report-qa');
(async()=>{
 fs.mkdirSync(out,{recursive:true});
 const manifest=JSON.parse(fs.readFileSync(path.join(report,'report-manifest.json')));
 const errors=[],requests=[],captures=[];
 const browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true,args:['--disable-background-networking']});
 const context=await browser.newContext({offline:true,viewport:{width:1440,height:1100},reducedMotion:'reduce'});const page=await context.newPage();
 page.on('pageerror',e=>errors.push(e.message));page.on('console',m=>{if(m.type()==='error')errors.push(m.text());});page.on('request',r=>{if(/^https?:/.test(r.url()))requests.push(r.url());});
 const hash=p=>crypto.createHash('sha256').update(fs.readFileSync(p)).digest('hex');
 const capture=async(name)=>{const p=path.join(out,name+'.png');await page.screenshot({path:p,animations:'disabled'});captures.push({path:p,sha256:hash(p)});};
 const overflow=async()=>assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),'Root page must not scroll horizontally');
 try{
  await page.goto(pathToFileURL(path.join(report,'index.html')).href);await page.evaluate(()=>document.fonts.ready);
  assert.equal(await page.locator('h1').count(),1);assert.equal(await page.locator('#rw-coverage-table tbody tr').count(),manifest.rw_cases);
  assert.equal(await page.locator('#modules-table tbody tr').count(),manifest.modules);assert.equal(await page.locator('#components-table tbody tr').count(),manifest.components);
  assert.equal(await page.locator('#rw-coverage-table td').filter({hasText:'not_run'}).count(),manifest.runtime_layer_gaps);
  assert.equal(await page.locator('#rw-coverage-table td').filter({hasText:'passed'}).count(),manifest.layer_counts.design);
  assert.match(await page.locator('#execution-proof').innerText(),/没有修改产品业务/);
  assert.match(await page.locator('#reading-window').innerText(),/契约草图|拟议/);
  assert.match(await page.locator('#reading-window').innerText(),/暂停|raw/);
  assert.equal(manifest.product_tests_executed,false);assert.equal(manifest.product_code_changed,false);assert.equal(manifest.release_status,'blocked');
  const statusCatalog=JSON.parse(fs.readFileSync(path.join(report,'references/docs/architecture/implementation-status.json')));
  assert.equal(manifest.implementation_catalog_sha256,hash(path.join(report,'references/docs/architecture/implementation-status.json')));
  assert.equal(statusCatalog.runtime_verified_this_audit,false);assert.equal(statusCatalog.current_release_approved,false);
  const statusTables=page.locator('#implementation-status table');assert.equal(await statusTables.count(),3);
  for(const [index,group] of ['capabilities','modules','components'].entries())assert.equal(await statusTables.nth(index).locator('tbody tr').count(),statusCatalog[group].length);
  assert.match(await page.locator('#implementation-status').innerText(),/当前实机.*未执行/);
  assert.match(await page.locator('#implementation-status').innerText(),/CAP-PREVIEW/);
  assert.match(await page.locator('#implementation-status').innerText(),/历史包范围失败/);
  assert.equal(await page.locator('#implementation-status select, #implementation-status input, #implementation-status script').count(),0,'Audit text must remain text, never become live controls or script');
  const statusRows=['capabilities','modules','components'].flatMap(group=>statusCatalog[group]);
  assert.equal(await page.locator('#implementation-status details').count(),statusRows.length);
  assert.equal(await page.locator('#implementation-status details a').count(),statusRows.reduce((count,row)=>count+row.source_evidence.length,0),'Every audited source reference must survive the collapsed view');
  const firstSource=page.locator('#implementation-status details').first();
  await firstSource.locator('summary').click();assert.equal(await firstSource.getAttribute('open'),'');
  await firstSource.locator('summary').click();assert.equal(await firstSource.getAttribute('open'),null);
  await page.locator('#implementation-status').scrollIntoViewIfNeeded();await capture('00-implementation-status-desktop');
  await page.evaluate(()=>scrollTo(0,0));await overflow();await capture('01-overview-desktop');
  await page.locator('#rw-coverage').scrollIntoViewIfNeeded();await capture('02-layered-coverage');
  await page.locator('#directory-map img').evaluate(img=>img.decode());assert.ok(await page.locator('#directory-map img').evaluate(img=>img.naturalWidth>0));
  await page.locator('#directory-map').scrollIntoViewIfNeeded();await capture('03-directory-map');
  // Search spans all CSV columns, including references from another component.
  const csvRows=JSON.parse(require('node:child_process').execFileSync('python3',['-c','import csv,json,sys; print(json.dumps(list(csv.DictReader(open(sys.argv[1],newline=""))),ensure_ascii=False))',path.join(report,'references/docs/architecture/engineering-components.csv')],{encoding:'utf8'}));
  for(const query of ['C35','C36']){
   await page.locator('#components input').fill(query);
   const expected=csvRows.filter(row=>Object.values(row).join(' ').toLocaleLowerCase().includes(query.toLocaleLowerCase())).length;
   assert.equal(await page.locator('#components-table tbody tr:visible').count(),expected,'Search must include every CSV column');
   assert.equal(await page.locator('#components-table tbody tr:visible').filter({has:page.locator('td:first-child').filter({hasText:new RegExp('^'+query+'$')})}).count(),1,'The requested component itself remains visible');
  }
  await page.locator('#components input').fill('');
  for(const href of await page.locator('a[href]').evaluateAll(nodes=>nodes.map(n=>n.getAttribute('href')))){
   if(!href||/^[a-z]+:/i.test(href))continue;
   if(href.startsWith('#')){assert.equal(await page.locator('[id="'+href.slice(1)+'"]').count(),1,'Missing anchor '+href);continue;}
   const target=path.resolve(report,decodeURIComponent(href.split('#')[0].split('?')[0]));assert.ok(fs.existsSync(target),'Missing offline target '+href);
  }
  for(const file of manifest.files){const p=path.join(report,file.path);assert.equal(hash(p),file.sha256,'Report artifact drift '+file.path);}
  await page.setViewportSize({width:390,height:844});await page.evaluate(()=>scrollTo(0,0));await overflow();await capture('04-overview-mobile');
  await page.locator('#implementation-status').scrollIntoViewIfNeeded();await overflow();await capture('00-implementation-status-mobile');
  await page.locator('#rw-coverage').scrollIntoViewIfNeeded();await overflow();await capture('05-coverage-mobile');
  await page.locator('#directory-map').scrollIntoViewIfNeeded();assert.ok(await page.locator('#directory-map .directory-viewport').evaluate(n=>n.scrollWidth>n.clientWidth));await overflow();await capture('06-directory-mobile');
  assert.deepEqual(errors,[]);assert.deepEqual(requests,[]);
  fs.writeFileSync(path.join(out,'verification.json'),JSON.stringify({status:'passed',scope:'offline report artifact QA',product_tests_executed:false,report,report_manifest_sha256:hash(path.join(report,'report-manifest.json')),captures,console_errors:errors,http_requests:requests},null,2)+'\n');
  console.log(JSON.stringify({status:'passed',captures:captures.length,out}));
 }finally{await context.close();await browser.close();}
})().catch(e=>{console.error(e.stack);process.exitCode=1;});
