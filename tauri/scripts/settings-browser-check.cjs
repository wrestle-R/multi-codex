// Real settings UI with fictional browser-mode data; no native IPC or account files.
const { chromium, webkit } = require('../../next/node_modules/playwright');
const assert = require('node:assert/strict');const fs = require('node:fs');
const path = require('node:path');const os = require('node:os');
const output = process.env.SETTINGS_EVIDENCE_DIR || fs.mkdtempSync(path.join(os.tmpdir(), 'multi-codex-settings-'));
fs.mkdirSync(output, { recursive: true });
(async()=>{
const runs=[];
for(const [engine,type] of Object.entries({chromium,webkit})){
 const browser=await type.launch();
 for(const viewport of [{width:1180,height:820},{width:480,height:520}])for(const mode of ['light','dark']){
  const page=await browser.newPage({viewport,reducedMotion:'reduce'});const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.addInitScript(mode=>{localStorage.setItem('multi-codex-theme',mode);localStorage.setItem('multi-codex-palette','sand');localStorage.setItem('multi-codex-executables',JSON.stringify({codePath:null,codexPath:'/opt/example/codex',globalCodexHome:null,preferredWorkspace:'/home/example/Projects',launchMode:'both',cliTerminal:'konsole'}));},mode);
  await page.goto(`${process.env.SETTINGS_BASE_URL || 'http://127.0.0.1:1420'}/?demo=1`);await page.getByRole('button',{name:'Launch settings',exact:true}).click();
  const dialog=page.getByRole('dialog',{name:'Folders and launching'});const picker=page.getByRole('combobox',{name:'CLI terminal'});await picker.click();
  const list=page.getByRole('listbox',{name:'CLI terminal'});assert.equal(await list.getByRole('option').count(),3);assert.equal(await list.getByRole('option',{name:/Ghostty/}).count(),0);
  const bounds=await page.locator('.terminal-picker-menu').boundingBox();const clip=await page.locator('.settings-body').boundingBox();
  assert(bounds.y>=clip.y-1&&bounds.y+bounds.height<=clip.y+clip.height+1,`Menu clipped: ${engine}/${mode}/${viewport.width} ${JSON.stringify({bounds,clip})}`);
  if(engine==='chromium'&&viewport.width===1180)await dialog.screenshot({path:path.join(output, `settings-${mode}.png`)});
  await picker.press('End');await picker.press('Enter');assert.match(await picker.textContent(),/Kitty/);await picker.click();await picker.press('Escape');await dialog.waitFor({state:'visible'});assert.equal(await list.count(),0);
  await page.locator('summary').filter({hasText:'Advanced'}).click();await page.getByLabel('Global Codex home',{exact:true}).scrollIntoViewIfNeeded();assert.equal(await page.getByRole('button',{name:'Save settings'}).isVisible(),true);
  const footer=await page.locator('.launch-settings-form .dialog-actions').boundingBox();assert(footer.y>=0&&footer.y+footer.height<=viewport.height);
  await page.getByRole('button',{name:'Save settings'}).click();await dialog.waitFor({state:'hidden'});
  const saved=await page.evaluate(()=>JSON.parse(localStorage.getItem('multi-codex-executables')));assert.equal(saved.cliTerminal,'kitty');assert.equal(saved.codexPath,'/opt/example/codex');assert.equal(saved.preferredWorkspace,'/home/example/Projects');assert.equal(saved.launchMode,'both');assert.equal(errors.length,0,errors.join('\n'));
  runs.push({engine,mode,viewport,installedOptionsOnly:true,menuUnclipped:true,keyboardAndEscape:true,advancedFooterReachable:true,unrelatedPreferencesPreserved:true,noPageErrors:true});await page.close();
 }
 await browser.close();
}
fs.writeFileSync(path.join(output, 'settings-browser-validation.json'),JSON.stringify({status:'passed',scenarios:runs.length,runs},null,2)+'\n');console.log(JSON.stringify({status:'passed',scenarios:runs.length}));
})().catch(e=>{console.error(e);process.exit(1)});
