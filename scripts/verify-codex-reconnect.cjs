// Own localhost provider and conversation only. Never attach to a user's daemon.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),http=require('node:http'),os=require('node:os');
const {createRequire}=require('node:module');
const {spawnSync}=require('node:child_process');
const dep=createRequire(path.join(process.env.APPDATA,'npm/node_modules/ai-battery/package.json'));
const pty=dep('node-pty'),{Terminal}=dep('@xterm/headless');
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
(async()=>{
 const root=path.resolve(__dirname,'../artifacts/reconnect-'+crypto.randomUUID());
 // Windows AF_UNIX daemon control sockets require a short CODEX_HOME.
 const home=path.join(process.env.LOCALAPPDATA,'mt-r-'+crypto.randomBytes(4).toString('hex'),'h'),work=path.join(root,'work');
 fs.mkdirSync(home,{recursive:true});fs.mkdirSync(work,{recursive:true});
 const exe=path.join(process.env.APPDATA,'npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe');
 // Cache only the installed public daemon binaries, never account/config/session data.
 const installed=path.join(process.env.USERPROFILE,'.codex/packages/app-server-daemon/releases/0.162.1-x86_64-pc-windows-msvc');
 const target=path.join(home,'packages/app-server-daemon/releases/0.162.1-x86_64-pc-windows-msvc');
 fs.cpSync(installed,target,{recursive:true});
 const requests=[];
 const server=http.createServer((req,res)=>{
  let body='';req.on('data',d=>body+=d);req.on('end',()=>{
   if(req.method!=='POST'||req.url!=='/v1/responses'){res.writeHead(404);res.end();return;}
   requests.push(JSON.parse(body));fs.writeFileSync(path.join(root,'requests.json'),JSON.stringify(requests,null,2));
   res.writeHead(400,{'content-type':'application/json'});res.end(JSON.stringify({error:{type:'invalid_request_error',message:'RECONNECT-FIXTURE-RECEIVED',code:'fixture'}}));
  });
 });
 await new Promise(r=>server.listen(0,'127.0.0.1',r));
 fs.writeFileSync(path.join(home,'config.toml'),`model="miniterm-reconnect-fixture"
model_provider="fixture"
approval_policy="never"
sandbox_mode="read-only"
cli_auth_credentials_store="file"
web_search="disabled"
[model_providers.fixture]
name="MiniTerm reconnect fixture"
base_url="http://127.0.0.1:${server.address().port}/v1"
wire_api="responses"
requires_openai_auth=false
supports_websockets=false
request_max_retries=0
stream_max_retries=0
[projects.'${path.resolve(__dirname,'..').toLowerCase()}']
trust_level="trusted"
`);
 const env={...process.env,CODEX_HOME:home,TERM:'xterm-256color',COLORTERM:'truecolor'};
 for(const n of ['OPENAI_API_KEY','CODEX_API_KEY','OPENAI_BASE_URL','MINITERM_SESSION_ID','MINITERM_NOTIFY_EXE','MINITERM_INSTANCE_ID','MINITERM_DIAGNOSTICS','MINITERM_AGENT_BASH','MINITERM_CODEX_SESSION_NOTIFY','NO_COLOR'])delete env[n];
 fs.writeFileSync(path.join(root,'fixture-owned.json'),JSON.stringify({kind:'miniterm-local-reconnect',root,home,work}));
 const children=[];
 function start(name,args){
  const terminal=new Terminal({cols:90,rows:35,allowProposedApi:true});
  const proc=pty.spawn(exe,args,{cols:90,rows:35,cwd:work,env});
  const child={name,proc,terminal,raw:'',chain:Promise.resolve(),exited:false};children.push(child);
  terminal.onData(d=>proc.write(d));proc.onData(d=>{child.raw+=d;child.chain=child.chain.then(()=>new Promise(r=>terminal.write(d,r)));});proc.onExit(()=>child.exited=true);
  return child;
 }
 async function screen(child){await child.chain;const b=child.terminal.buffer.active;return Array.from({length:child.terminal.rows},(_,r)=>b.getLine(b.viewportY+r)?.translateToString(true)||'').join('\n');}
 async function waitFor(child,text){const end=Date.now()+20000;while(Date.now()<end&&!child.exited){const s=await screen(child);if(s.includes(text))return s;await pause(25);}throw new Error(child.name+' missing '+text);}
 function key(child,vk,scan,unicode){for(const down of [1,0])child.proc.write('\x1b['+[vk,scan,unicode,down,0,1].join(';')+'_');}
 async function submit(child,text){
  child.proc.write('\x1b[200~'+text+'\x1b[201~');
  // Make Enter a separate key event after Codex finishes consuming the paste.
  await pause(150);key(child,13,28,13);
 }
 async function waitForInput(text){const end=Date.now()+20000;while(Date.now()<end){if(requests.some(r=>(r.input||[]).some(i=>i.role==='user'&&(i.content||[]).some(c=>c.text===text))))return;await pause(25);}throw new Error('Provider missing '+text);}
 async function close(child){if(child.exited)return;child.proc.write('\x03');await pause(150);child.proc.write('\x03');const end=Date.now()+3000;while(!child.exited&&Date.now()<end)await pause(30);if(!child.exited)child.proc.kill();}
 const result={root,scope:'Installed CLI; own managed daemon, own thread and HTTP400 provider. No account/model or user daemon mutation.'};
 try{
  const owner=start('owner',['--no-alt-screen','RECONNECT-OWNED-FIRST']);
  await waitForInput('RECONNECT-OWNED-FIRST');
  const {DatabaseSync}=require('node:sqlite');const db=new DatabaseSync(path.join(home,'state_5.sqlite'));
  let thread;try{thread=db.prepare('SELECT id FROM threads ORDER BY created_at DESC LIMIT 1').get().id;}finally{db.close();}
  result.thread=thread;
  const isolated=start('isolated',['--no-alt-screen','--no-daemon','resume',thread]);
  await waitFor(isolated,'This conversation is open in another app');
  key(isolated,82,19,114);await pause(700);
  result.privateBackendBlocked=(await screen(isolated)).includes('This conversation is open in another app');
  await close(owner);key(isolated,82,19,114);await pause(700);
  result.ownerFrontendClosedButDaemonStillBlocks=(await screen(isolated)).includes('This conversation is open in another app');
  const shared=start('shared',['--no-alt-screen','--remote','unix://','-c','features.unified_exec=true','-c','features.terminal_visualization_instructions=true','-c','developer_instructions=MINITERM-SHARED-BASH-GUIDANCE','resume',thread,'RECONNECT-SHARED-PROMPT']);
  await waitForInput('RECONNECT-SHARED-PROMPT');
  result.sharedRejoinsExisting=!(await screen(shared)).includes('This conversation is open in another app');
  result.sharedInputReceived=requests.some(r=>(r.input||[]).some(i=>i.role==='user'&&(i.content||[]).some(c=>c.text==='RECONNECT-SHARED-PROMPT')));
  result.sharedBashGuidanceReceived=requests.some(r=>JSON.stringify(r).includes('MINITERM-SHARED-BASH-GUIDANCE'));
  await close(shared);
  // Release only this fixture's daemon, then verify lowercase r recovery.
  const stopped=spawnSync(exe,['app-server','daemon','stop'],{env,cwd:work,windowsHide:true,encoding:'utf8',timeout:15000});
  if(stopped.status!==0)throw new Error('Own daemon stop failed: '+stopped.stderr);
  key(isolated,82,19,114);await waitFor(isolated,'Ask Codex');
  result.retryAfterOwnerRelease=!(await screen(isolated)).includes('This conversation is open in another app');
 }catch(error){result.error=error.message;}
 finally{
  for(const child of children){fs.writeFileSync(path.join(root,child.name+'.vt'),child.raw);fs.writeFileSync(path.join(root,child.name+'-screen.txt'),await screen(child));await close(child);}
  // CODEX_HOME always points to the marker-validated directory we just created.
  const stopped=spawnSync(exe,['app-server','daemon','stop'],{env,cwd:work,windowsHide:true,encoding:'utf8',timeout:15000});
  result.ownedDaemonStopped=stopped.status===0;
  result.passed=!result.error&&result.privateBackendBlocked&&result.ownerFrontendClosedButDaemonStillBlocks&&result.sharedRejoinsExisting&&result.sharedInputReceived&&result.sharedBashGuidanceReceived&&result.retryAfterOwnerRelease;
  fs.writeFileSync(path.join(root,'verification.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));process.exitCode=result.passed?0:1;
  // node-pty console handles can remain referenced after all child exits.
  for(const child of children)child.terminal.dispose();
  server.close(()=>process.exit(process.exitCode));
 }
})().catch(e=>{console.error(e);process.exitCode=1;});
