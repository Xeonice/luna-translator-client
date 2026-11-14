/* QA for the generated review document; not an application/runtime acceptance. */
const {chromium}=require('playwright-core');
const fs=require('node:fs');
const path=require('node:path');
const assert=require('node:assert/strict');
const {pathToFileURL}=require('node:url');
(async()=>{
 const output=path.resolve(process.argv[2]||'.build/design-six-gaps-20261019/report-qa');fs.mkdirSync(output,{recursive:true});
 const browser=await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true,args:['--disable-background-networking']});
 const failures=[],requests=[],checks=[];
 try{
  for(const width of [1280,390]){
   const context=await browser.newContext({offline:true,viewport:{width,height:900}});const page=await context.newPage();
   page.on('pageerror',error=>failures.push(error.message));page.on('console',message=>{if(message.type()==='error')failures.push(message.text());});page.on('request',request=>{if(/^https?:/.test(request.url()))requests.push(request.url());});
   await page.goto(pathToFileURL(path.resolve('docs/design/first-product-v1/interaction-review.html')).href);
   await page.evaluate(async()=>{for(const img of document.images)img.loading='eager';await Promise.all([...document.images].map(img=>img.complete?Promise.resolve():new Promise(resolve=>{img.onload=resolve;img.onerror=resolve;})));});
   assert.equal(await page.locator('h1').textContent(),'把缺失的操作补进完整旅程。');
   assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>window.innerWidth),false,'No horizontal document overflow');
   const images=await page.locator('img').evaluateAll(nodes=>nodes.map(img=>({src:img.getAttribute('src'),ready:img.complete&&img.naturalWidth>0})));
   assert.ok(images.length>=35);assert.ok(images.every(img=>img.ready),'Every candidate/reference image must load');
   assert.match(await page.locator('main').textContent(),/不执行真实游戏/);
   assert.match(await page.locator('main').textContent(),/更多只留低频/);
   await page.screenshot({path:path.join(output,`review-top-${width}.png`)});
   checks.push({viewport:{width,height:900},status:'passed',images:images.length,assertions:['candidate heading and scope','no horizontal overflow','all local candidate/reference images loaded','HUD toolbar/no duplicate menu described']});
   await context.close();
  }
  assert.deepEqual(failures,[]);assert.deepEqual(requests,[]);
  const result={kind:'candidate_review_document_qa_not_product',status:'passed',checks,browser_errors:failures,http_requests:requests};fs.writeFileSync(path.join(output,'verification.json'),JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(result));
 }finally{await browser.close();}
})().catch(error=>{console.error(error.stack);process.exitCode=1;});
