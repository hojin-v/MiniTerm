// Requires the installed Windows Codex CLI and AI Battery's node-pty/xterm dependencies.
// All transcripts, settings and the HTTP error fixture belong to a fresh test directory.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const http=require('node:http');
const {createRequire} = require('node:module');
const batteryRequire = createRequire(path.join(process.env.APPDATA, 'npm/node_modules/ai-battery/package.json'));
const pty = batteryRequire('node-pty');
const {Terminal} = batteryRequire('@xterm/headless');
(async()=>{
const server=http.createServer((request,response)=>{
  let body='';
  request.on('data',chunk=>{if(body.length<4*1024*1024)body+=chunk.toString('utf8');});
  request.on('end',()=>{
    const correlation=body.match(/__MINITERM_UI_REQUEST_\d+__/g)?.at(-1)||'';
    response.writeHead(400,{'content-type':'application/json'});
    response.end(JSON.stringify({error:{message:'MiniTerm local UI fixture; no model executed. Fixture response '+correlation,type:'invalid_request_error',code:'fixture'}}));
  });
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const encoding=process.argv[2]||'plain';
if (!['plain','win32'].includes(encoding)) throw new Error('Expected plain or win32 key encoding');
const wrapped=['battery','bash-battery'].includes(process.argv[3]);
const throughBash=['bash','bash-battery'].includes(process.argv[3]);
const scenario=process.argv[4]||'menus';
if(!['menus','goal-seed','goal-resume'].includes(scenario)) throw new Error('Unexpected UI scenario');
const root = path.resolve(__dirname, '../artifacts/codex-ui-' + crypto.randomUUID());
const home = path.join(root, 'codex-home');
let work = path.join(root, 'work');
fs.mkdirSync(home, {recursive:true}); fs.mkdirSync(work, {recursive:true});
fs.writeFileSync(path.join(root,'fixture-owned.json'),JSON.stringify({kind:'miniterm-local-codex-ui',root}));
if(scenario==='goal-resume') {
  // Only clone a completed local fixture created by this harness, never user sessions.
  const artifacts=fs.realpathSync(path.resolve(__dirname,'../artifacts'));
  const seedRoot=fs.realpathSync(process.argv[5]);
  if(path.dirname(seedRoot)!==artifacts || !/^codex-ui-[0-9a-f-]{36}$/.test(path.basename(seedRoot))) throw new Error('Expected an owned UI seed');
  const marker=JSON.parse(fs.readFileSync(path.join(seedRoot,'fixture-owned.json'),'utf8'));
  const seedResult=JSON.parse(fs.readFileSync(path.join(seedRoot,'result.json'),'utf8'));
  const seed=path.join(seedRoot,'codex-home');
  if(marker.kind!=='miniterm-local-codex-ui' || marker.root!==seedRoot || seedResult.scenario!=='goal-seed' || seedResult.error || !seedResult.exited || fs.existsSync(path.join(seed,'auth.json'))) throw new Error('Seed is not a completed account-free fixture');
  fs.cpSync(seed,home,{recursive:true});
  const {DatabaseSync}=require('node:sqlite');
  const database=new DatabaseSync(path.join(home,'state_5.sqlite'));
  try { database.prepare('UPDATE threads SET rollout_path=replace(rollout_path, ?, ?)').run(seed,home); }
  finally { database.close(); }
  work=path.join(seedRoot,'work');
  fs.writeFileSync(path.join(root,'seed.json'),JSON.stringify({source:seedRoot,scope:'Copied only a completed owned fixture. User sessions and credentials are excluded.'},null,2));
}
const workToml = work.replaceAll('\\', '/');
fs.writeFileSync(path.join(home, 'config.toml'), `model = "miniterm-ui-fixture"
model_provider = "fixture"
approval_policy = "never"
sandbox_mode = "read-only"
cli_auth_credentials_store = "file"
web_search = "disabled"
project_root_markers = []
[model_providers.fixture]
name = "MiniTerm UI Fixture"
base_url = "http://127.0.0.1:${server.address().port}/v1"
wire_api = "responses"
requires_openai_auth = false
supports_websockets = false
request_max_retries = 0
stream_max_retries = 0
[projects."${workToml}"]
trust_level = "trusted"
`);
const env={...process.env, CODEX_HOME:home, TERM:'xterm-256color', COLORTERM:'truecolor'};
for(const name of ['NO_COLOR','OPENAI_API_KEY','CODEX_API_KEY','OPENAI_BASE_URL','MINITERM_SESSION_ID','MINITERM_INSTANCE_ID','MINITERM_DIAGNOSTICS','MINITERM_DATA_DIR']) delete env[name];
const exe=path.join(process.env.APPDATA, 'npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe');
const battery=path.join(root,'battery-fixture.cjs');
fs.writeFileSync(battery,`process.stdout.write('AI Battery \\x1b[32m❚❚❚❚\\x1b[90m❚❚\\x1b[0m 67%');`);
env.AI_BATTERY_BIN=battery; env.AI_BATTERY_DATA_DIR=path.join(root,'battery-data');
env.AI_BATTERY_RUNTIME_DOWNLOAD='0'; env.AI_BATTERY_DEBUG_LOG=path.join(root,'battery-debug.jsonl');
const node=path.join(process.env.LOCALAPPDATA,'ai-battery/runtimes/node-v24.21.0-win-x64/node.exe');
const runner=path.join(process.env.APPDATA,'npm/node_modules/ai-battery/bin/ai-battery-run-win.js');
const codexArgs=['--no-daemon','--strict-config'];
const inline=process.env.MINITERM_CODEX_UI_INLINE==='1';
if(inline)codexArgs.push('--no-alt-screen');
if(scenario==='goal-resume') codexArgs.push('resume','--last','--all');
const initialRows=throughBash?24:52;
const shellQuote=text=>"'"+text.replaceAll("'","'\\''")+"'";
if(throughBash) { env.PS1='__MINITERM_UI_BASH__ ';env.HISTFILE='/dev/null'; }
const launchExe=throughBash?path.join(process.env.ProgramFiles,'Git/usr/bin/bash.exe'):wrapped?node:exe;
const launchArgs=throughBash?['--noprofile','--norc','-i']:wrapped?[runner,'--layout','composite','--',exe,...codexArgs]:codexArgs;
const proc=pty.spawn(launchExe,launchArgs,{cols:80,rows:initialRows,cwd:work,env});
fs.writeFileSync(path.join(root,'process.json'),JSON.stringify({launcherPid:process.pid,ptyPid:proc.pid,launchExe,throughBash},null,2));
const terminal=new Terminal({cols:80,rows:initialRows,allowProposedApi:true,scrollback:1000});
let raw='',exited=false,parseChain=Promise.resolve();
let uiRequest=0;
const checkpoints=[];
const pause=ms=>new Promise(r=>setTimeout(r,ms));
terminal.onData(data=>proc.write(data));
proc.onData(data=>{raw+=data; parseChain=parseChain.then(()=>new Promise(r=>terminal.write(data,r)));});
proc.onExit(()=>{exited=true;});
async function screen() {
  await parseChain;
  const b=terminal.buffer.active;
  return Array.from({length:terminal.rows},(_,n)=>b.getLine(b.viewportY+n)?.translateToString(true)||'');
}
async function waitFor(text) {
  const deadline=Date.now()+12000;
  while(Date.now()<deadline && !exited) { const rows=await screen(); if(rows.join('\n').replace(/\s+/g,' ').includes(text.replace(/\s+/g,' ')))return rows; await pause(20); }
  throw new Error('Missing '+text);
}
async function capture(name) {
  const rows=await screen();
  const styles=[];
  const buffer=terminal.buffer.active;
  for(let row=0;row<terminal.rows;row++) for(let column=0;column<terminal.cols;column++) {
    const cell=buffer.getLine(buffer.viewportY+row)?.getCell(column);
    if(!cell || !cell.getChars() || cell.getWidth()===0) continue;
    const bold=Boolean(cell.isBold()), dim=Boolean(cell.isDim()), italic=Boolean(cell.isItalic()), inverse=Boolean(cell.isInverse());
    if(bold || dim || italic || inverse || cell.isBgRGB() || cell.isBgPalette() || cell.isFgRGB() || cell.isFgPalette()) {
      styles.push({row,column,bold,dim,italic,inverse,fg:cell.isFgRGB()?{rgb:cell.getFgColor()}:cell.isFgPalette()?{index:cell.getFgColor()}:null,bg:cell.isBgRGB()?{rgb:cell.getBgColor()}:cell.isBgPalette()?{index:cell.getBgColor()}:null});
    }
  }
  const checkpoint={name,offset:Buffer.byteLength(raw),columns:terminal.cols,rows:terminal.rows,cursor:[buffer.cursorX,buffer.cursorY],screen:rows,styles};
  checkpoints.push(checkpoint);
  fs.writeFileSync(path.join(root,name+'.json'),JSON.stringify(checkpoint,null,2));
  if(name!=='failure' && process.env.MINITERM_CODEX_UI_EXPECT_DOCKED==='1' && scenario==='goal-resume' && rows.some(row=>row.includes('Resume paused goal?'))) {
    const titleRow=rows.findIndex(row=>row.includes('Resume paused goal?'));
    const footerRow=rows.findIndex(row=>row.includes('enter select') && row.includes('esc back'));
    const bottom=terminal.rows-(wrapped?2:1);
    if(titleRow<bottom-11 || Math.abs(footerRow-bottom)>1)throw new Error('Goal chooser is floating in the transcript: title row '+titleRow+', footer row '+footerRow+', bottom '+bottom);
  }
  return rows;
}
function key(vk,sc,uc) {
  if(encoding==='plain') { proc.write(String.fromCharCode(uc)); return; }
  proc.write('\x1b['+[vk,sc,uc,1,0,1].join(';')+'_\x1b['+[vk,sc,uc,0,0,1].join(';')+'_');
}
async function command(text,marker) {
  if(marker==='MiniTerm local UI fixture') {
    const correlation='__MINITERM_UI_REQUEST_'+(++uiRequest)+'__';
    text+=' '+correlation;
    marker='Fixture response '+correlation;
  }
  proc.write(text); await pause(150); key(13,28,13);
  await waitFor(marker); await pause(100);
}
async function escape() {
  key(27,1,27); await waitFor('Ask Codex'); await pause(100);
  const text=(await screen()).join('\n');
  if(text.includes('Select Model and Effort') || text.includes('Resume a previous session')) throw new Error('Dismissed menu remains visible');
}
async function resize(columns,rows,name,marker) {
  checkpoints.push({resize:true,offset:Buffer.byteLength(raw),columns,rows});
  proc.resize(columns,rows); terminal.resize(columns,rows); await pause(450);
  await waitFor(marker); await capture(name);
  if(wrapped && !(await screen()).at(-1).startsWith('AI Battery')) throw new Error('Battery did not follow the resized bottom row');
}
await (async()=>{
  let error=null;
  try {
    if(throughBash) {
      await waitFor('__MINITERM_UI_BASH__');
      const argv=wrapped?[node,runner,'--layout','composite','--',exe,...codexArgs]:[exe,...codexArgs];
      proc.write(argv.map(value=>shellQuote(value.replaceAll('\\','/'))).join(' ')+'\r');
    }
    if(scenario==='goal-resume') {
      await waitFor('Resume paused goal?');await pause(400);await capture('paused-goal-menu');
      if(throughBash) await resize(80,52,'paused-goal-expanded','Resume paused goal?');
      await resize(60,24,'paused-goal-small','Resume paused goal?');
      await resize(96,48,'paused-goal-large','Resume paused goal?');
      proc.write('\x1b[B');await pause(200);await capture('leave-paused-selected');key(13,28,13);
      await waitFor('Ask Codex');await pause(300);await capture('paused-goal-closed');
      if((await screen()).join('\n').includes('Resume paused goal?')) throw new Error('Dismissed paused goal menu remains');
      await command('닫힌 선택 메뉴 이후 한글 시험','MiniTerm local UI fixture');await pause(250);await capture('turn-after-goal-menu');
      if((await screen()).join('\n').includes('Resume paused goal?')) throw new Error('Paused goal menu reappeared in history');
      if(wrapped && !(await screen()).at(-1).startsWith('AI Battery')) throw new Error('Battery is not on the final terminal row');
    } else if(scenario==='goal-seed') {
      await waitFor('OpenAI Codex');await pause(800);await capture('startup');
      for(let n=0;n<5;n++) {
        await command('한글 선택 메뉴 회귀 시험 '+n+' '+('결과 영역과 입력 영역의 위치를 확인합니다. '.repeat(5)),'MiniTerm local UI fixture');
        await pause(250);
      }
      await capture('history-before-goal');
      await command('/goal 한글 메뉴의 표시와 닫기 동작을 확인','Goal active');
      await waitFor('Goal stalled');
      await command('/goal pause','Goal paused');await capture('goal-paused');
    } else {
    await waitFor('OpenAI Codex'); await pause(800); await capture('startup');
    if(wrapped) await waitFor('AI Battery');
    await command('한글 세션명 검증 ABC 123','MiniTerm local UI fixture'); await capture('korean-local-turn');
    await command('/model','Select Model and Effort'); await capture('model-menu');
    await escape(); await capture('model-closed');
    await command('/resume','Resume a previous session'); await waitFor('한글 세션명 검증'); await capture('resume-menu');
    if(wrapped && !(await screen()).at(-1).startsWith('AI Battery')) throw new Error('Battery is not on the last terminal row');
    await escape(); await capture('resume-closed');
    checkpoints.push({resize:true,offset:Buffer.byteLength(raw),columns:80,rows:36});
    proc.resize(80,36); terminal.resize(80,36); await pause(350);
    await command('/model','Select Model and Effort'); await capture('model-menu-resized');
    await escape(); await capture('model-closed-resized');
    if(wrapped && !(await screen()).at(-1).startsWith('AI Battery')) throw new Error('Battery did not follow the resized bottom row');
    await command('/resume','Resume a previous session'); await waitFor('한글 세션명 검증'); await capture('resume-before-resize');
    await resize(60,24,'resume-small','Resume a previous session');
    await resize(96,48,'resume-large','Resume a previous session');
    await escape(); await capture('resume-closed-large');
    }
  } catch(e) { error=e.message; await capture('failure'); }
  finally {
    proc.write('\x03'); await pause(200); proc.write('\x03');
    if(throughBash) {
      for(let attempt=0;attempt<4 && !exited;attempt++) {
        await pause(250);
        await parseChain;
        const buffer=terminal.buffer.active;
        if(buffer.getLine(buffer.baseY+buffer.cursorY)?.translateToString(true).trim().startsWith('__MINITERM_UI_BASH__')) { proc.write('exit\r');break; }
        proc.write('\x03');
      }
    }
    const deadline=Date.now()+3000; while(!exited && Date.now()<deadline)await pause(20);
    if(!exited)proc.kill();
    fs.writeFileSync(path.join(root,'output.vt'),raw);
    fs.writeFileSync(path.join(root,'checkpoints.json'),JSON.stringify(checkpoints,null,2));
    fs.writeFileSync(path.join(root,'result.json'),JSON.stringify({root,encoding,wrapped,throughBash,inline,expectDocked:process.env.MINITERM_CODEX_UI_EXPECT_DOCKED==='1',scenario,error,exited,scope:'Native Codex TUI with isolated config and a localhost provider that always returns HTTP 400; no model executed. Goal resume uses a copied owned paused-goal fixture. Optional installed AI Battery compositor uses a fixed local battery reading and does not read account usage. Optional no-profile Bash starts at 24 rows and expands while the goal menu is open. Excludes live tool output and GPU pixels.'},null,2));
    console.log(JSON.stringify({root,encoding,wrapped,throughBash,inline,scenario,error,exited}));
    server.close();
    process.exit(error?1:0);
  }
})();
})().catch(error=>{console.error(error);process.exit(1);});
