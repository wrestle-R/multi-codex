// Real settings UI with fictional browser-mode data; no native IPC or account files.
const { chromium, webkit } = require('../../next/node_modules/playwright');
const assert = require('node:assert/strict');const fs = require('node:fs');
const path = require('node:path');const os = require('node:os');
const output = process.env.SETTINGS_EVIDENCE_DIR || fs.mkdtempSync(path.join(os.tmpdir(), 'multi-codex-settings-'));
fs.mkdirSync(output, { recursive: true });
(async()=>{
const runs=[];
const engines=Object.entries({chromium,webkit}).filter(([name])=>!process.env.SETTINGS_BROWSERS||process.env.SETTINGS_BROWSERS.split(',').includes(name));
assert(engines.length,'Choose an available browser engine');
for(const [engine,type] of engines){
 const browser=await type.launch();
 for(const viewport of [{width:1180,height:820},{width:480,height:520}])for(const mode of ['light','dark']){
  const page=await browser.newPage({viewport,reducedMotion:'reduce'});const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.addInitScript(mode=>{localStorage.setItem('multi-codex-theme',mode);localStorage.setItem('multi-codex-palette','sand');localStorage.setItem('multi-codex-executables',JSON.stringify({codePath:null,codexPath:'/opt/example/codex',globalCodexHome:null,preferredWorkspace:'/home/example/Projects',launchMode:'both',cliTerminal:'konsole'}));},mode);
  await page.goto(`${process.env.SETTINGS_BASE_URL || 'http://127.0.0.1:1420'}/?demo=1`);await page.getByRole('button',{name:'Settings',exact:true}).click();
  const dialog=page.getByRole('dialog',{name:'Settings'});
  const workspace=await page.getByRole('region',{name:'Workspace preferences'}).boundingBox();
  const updates=await page.getByRole('region',{name:'Update preferences'}).boundingBox();
  if(viewport.width>760){assert(updates.x>workspace.x+workspace.width,'Desktop settings did not use two columns');assert(Math.abs(updates.y-workspace.y)<3,'Desktop columns must align');assert((await dialog.boundingBox()).width>=900,'Settings did not become wider');}
  else assert(Math.abs(updates.x-workspace.x)<3,'Narrow settings must use one column');
  const picker=page.getByRole('combobox',{name:'CLI terminal'});await picker.click();
  const list=page.getByRole('listbox',{name:'CLI terminal'});assert.equal(await list.getByRole('option').count(),3);assert.equal(await list.getByRole('option',{name:/Ghostty/}).count(),0);
  const bounds=await page.locator('.terminal-picker-menu').boundingBox();const clip=await page.locator('.settings-body').boundingBox();
  assert(bounds.y>=clip.y-1&&bounds.y+bounds.height<=clip.y+clip.height+1,`Menu clipped: ${engine}/${mode}/${viewport.width} ${JSON.stringify({bounds,clip})}`);
  if(engine==='chromium'&&viewport.width===1180)await dialog.screenshot({path:path.join(output, `settings-${mode}.png`)});
  await picker.press('End');await picker.press('Enter');assert.match(await picker.textContent(),/Kitty/);await picker.click();await picker.press('Escape');await dialog.waitFor({state:'visible'});assert.equal(await list.count(),0);
  await page.locator('summary').filter({hasText:'Advanced'}).click();await page.getByLabel('Global Codex home',{exact:true}).scrollIntoViewIfNeeded();assert.equal(await page.getByRole('button',{name:'Save settings'}).isVisible(),true);
  const footer=await page.locator('.launch-settings-form .dialog-actions').boundingBox();assert(footer.y>=0&&footer.y+footer.height<=viewport.height);
  await page.getByRole('button',{name:'Save settings'}).click();await dialog.waitFor({state:'hidden'});
  const saved=await page.evaluate(()=>JSON.parse(localStorage.getItem('multi-codex-executables')));assert.equal(saved.cliTerminal,'kitty');assert.equal(saved.codexPath,'/opt/example/codex');assert.equal(saved.preferredWorkspace,'/home/example/Projects');assert.equal(saved.launchMode,'both');assert.equal(errors.length,0,errors.join('\n'));
  await page.goto(`${process.env.SETTINGS_BASE_URL || 'http://127.0.0.1:1420'}/?demo=1&releaseNotes=1&previousVersion=1.6.1`);
  const notes=page.getByRole('dialog',{name:'What’s new'});await notes.waitFor();
  assert.equal(await notes.getByRole('region',{name:'Release 1.7.1'}).count(),1);
  assert.equal(await notes.getByRole('region',{name:'Release 1.7.0'}).count(),1);
  assert.equal(await notes.getByRole('region',{name:'Release 1.6.1'}).count(),0);
  const dismiss=notes.getByRole('button',{name:'Got it'});const dismissBounds=await dismiss.boundingBox();assert(dismissBounds.y>=0&&dismissBounds.y+dismissBounds.height<=viewport.height,'Release history dismissal is clipped');
  if(engine==='chromium'&&viewport.width===1180)await notes.screenshot({path:path.join(output, `release-history-${mode}.png`)});
  await dismiss.click();await notes.waitFor({state:'hidden'});
  await page.getByRole('button',{name:/Version 1.7.1/}).click();await notes.waitFor();assert.equal(await notes.getByRole('region',{name:'Release 1.7.0'}).count(),1);
  runs.push({engine,mode,viewport,responsiveColumns:true,skippedReleaseHistory:true,installedOptionsOnly:true,menuUnclipped:true,keyboardAndEscape:true,advancedFooterReachable:true,unrelatedPreferencesPreserved:true,noPageErrors:true});await page.close();
 }
 await browser.close();
}
fs.writeFileSync(path.join(output, 'settings-browser-validation.json'),JSON.stringify({status:'passed',scenarios:runs.length,runs},null,2)+'\n');console.log(JSON.stringify({status:'passed',scenarios:runs.length}));
})().catch(e=>{console.error(e);process.exit(1)});
