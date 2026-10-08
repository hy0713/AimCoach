// Actual Tauri WebView2 + actual Rust commands + isolated SQLite. No mock IPC.
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdirSync,readFileSync,writeFileSync } from 'node:fs';
import { resolve,join } from 'node:path';
import assert from 'node:assert/strict';
const root=resolve(import.meta.dirname,'..');
const output=join(root,'test-output',`native-${Date.now()}`);
const stats=join(output,'stats');mkdirSync(stats,{recursive:true});
const env={...process.env,AIMCOACH_DATA_DIR:join(output,'data'),WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:'--remote-debugging-port=15422'};
const build=process.argv.includes('--release')?'release':'debug';
const child=spawn(join(root,'src-tauri','target',build,'aimcoach.exe'),[],{env,cwd:root,windowsHide:true,stdio:'pipe'});
let browser;
try{
  for(let i=0;i<80;i++){try{const r=await fetch('http://127.0.0.1:15422/json/version');if(r.ok)break;}catch{}await new Promise(r=>setTimeout(r,250));}
  browser=await chromium.connectOverCDP('http://127.0.0.1:15422');
  const context=browser.contexts()[0];let page=context.pages()[0];
  if(!page)page=await context.waitForEvent('page',{timeout:20000});
  await page.waitForSelector('.brand',{timeout:25000});
  const call=(cmd,args)=>page.evaluate(({cmd,args})=>window.__TAURI_INTERNALS__.invoke(cmd,args),{cmd,args});
  let snap=await call('snapshot');assert.equal(snap.scenarios.length,0);
  for(const label of ['场景与成长','能力基准','今天练什么','智能教练','设置']){await page.locator('nav button').filter({hasText:label}).click();assert.equal(await page.locator('header h1').textContent(),label);}
  await call('save_settings',{settings:{...snap.settings,stats_dir:stats,auto_watch:true}});
  const sample=readFileSync(join(root,'tests','fixtures','Synthetic Control α - Challenge - 2026.10.01-18.30.00 Stats.csv'),'utf8');
  writeFileSync(join(stats,'first.csv'),sample);
  for(let i=0;i<60;i++){snap=await call('snapshot');if(snap.scenarios.length===1)break;await new Promise(r=>setTimeout(r,250));}
  assert.equal(snap.scenarios.length,1);assert.equal(snap.scenarios[0].count,1);assert.ok(snap.watching);
  writeFileSync(join(stats,'second.csv'),sample.replace('18.30.00','18.31.00').replace('Score:,950','Score:,1000'));
  for(let i=0;i<60;i++){snap=await call('snapshot');if(snap.scenarios[0]?.count===2)break;await new Promise(r=>setTimeout(r,250));}
  assert.equal(snap.scenarios[0].count,2);assert.ok(snap.reports.length>0);
  const repeat=await call('scan_stats',{force:false});assert.equal(repeat.imported,0);assert.equal(repeat.skipped,2);
  await call('import_benchmark',{json:readFileSync(join(root,'tests','fixtures','benchmark.synthetic.json'),'utf8')});
  const bv=await call('snapshot');assert.equal(bv.benchmark.rows[0].best.rank,'Test Silver');
  const local=await call('generate_report',{useAi:false});assert.equal(local.kind,'local');
  await call('backup_database',{target:join(output,'backup.sqlite')});
  await page.locator('nav button').filter({hasText:'场景与成长'}).click();
  await page.getByText('Synthetic Control α',{exact:true}).first().click();
  await page.getByText('历史成绩曲线',{exact:true}).waitFor();
  await page.locator('canvas').first().waitFor();
  await page.screenshot({path:join(output,'native-history.png'),fullPage:true});
  snap=await call('snapshot');
  await call('save_settings',{settings:{...snap.settings,auto_watch:false}});
  for(let i=0;i<30;i++){snap=await call('snapshot');if(!snap.watching)break;await new Promise(r=>setTimeout(r,100));}
  assert.equal(snap.watching,false);
  await page.locator('.sync-label').filter({hasText:'手动同步'}).waitFor();
  const result={build,native_window:true,empty_navigation:true,actual_watcher_imported:2,watcher_disable_ui:true,repeat_imported:repeat.imported,repeat_skipped:repeat.skipped,benchmark:true,local_report:true,backup:true,history_chart:true,output};
  writeFileSync(join(output,'result.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result,null,2));
}finally{if(browser)await browser.close();child.kill();}
