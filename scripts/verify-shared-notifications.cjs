// Installed Codex TUI + production GUI IPC, without desktop keyboard/mouse input.
// All servers, conversations, callbacks and processes belong to this fixture.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),http=require('node:http'),net=require('node:net');
const {spawn,spawnSync}=require('node:child_process'),{createRequire}=require('node:module');
// Prefer isolated test dependencies; the optional AI Battery installation can
// change independently of MiniTerm and must not be reinstalled by this test.
const localDeps=path.resolve(__dirname,'../.tools/feature-test-node/package.json');
const dep=createRequire(fs.existsSync(localDeps)?localDeps:path.join(process.env.APPDATA,'npm/node_modules/ai-battery/package.json'));
const pty=dep('node-pty'),{Terminal}=dep('@xterm/headless');
const pause=ms=>new Promise(r=>setTimeout(r,ms));
(async()=>{
 const helper=path.resolve(process.argv[2]||'artifacts/miniterm.exe');
 const root=path.resolve(__dirname,'../artifacts/shared-bash-'+crypto.randomUUID());
 const home=path.join(process.env.LOCALAPPDATA,'mt-b-'+crypto.randomBytes(4).toString('hex'),'h'),work=path.join(root,'work');
 fs.mkdirSync(home,{recursive:true});fs.mkdirSync(path.join(work,'.git'),{recursive:true});
 const native=path.join(process.env.APPDATA,'npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe');
 const nativeVersionResult=spawnSync(native,['--version'],{windowsHide:true,encoding:'utf8',timeout:10000});
 const nativeVersion=/^codex-cli (\d+\.\d+\.\d+)/.exec(nativeVersionResult.stdout||'')?.[1];
 if(nativeVersionResult.status!==0||!nativeVersion)throw new Error('Cannot identify the installed native Codex fixture version');
 const daemonRelease=nativeVersion+'-x86_64-pc-windows-msvc';
 fs.cpSync(path.join(process.env.USERPROFILE,'.codex/packages/app-server-daemon/releases',daemonRelease),path.join(home,'packages/app-server-daemon/releases',daemonRelease),{recursive:true});
 fs.writeFileSync(path.join(root,'fixture-owned.json'),JSON.stringify({kind:'miniterm-local-shared-bash',root,home,work}));
 const requests=[];
 const server=http.createServer((req,res)=>{
  let body='';req.on('data',d=>body+=d);req.on('end',()=>{
   if(req.url!=='/v1/responses'||req.method!=='POST'){res.writeHead(404);res.end();return;}
   const request=JSON.parse(body);requests.push(request);
   const user=JSON.stringify((request.input||[]).filter(i=>i.role==='user').at(-1));
   const tag=['FINAL','RECOVERED','RESUME'].find(t=>user?.includes('LINK-'+t))||'FIRST';
   const call='call_link_'+tag.toLowerCase();
   const output=(request.input||[]).some(i=>i.type==='function_call_output'&&i.call_id===call);
   const tool=(request.tools||[]).find(t=>t.name==='exec_command'||t.name==='shell_command');
   const item=tool&&!output?{type:'function_call',id:'fc_'+tag,call_id:call,name:tool.name,arguments:JSON.stringify({[tool.name==='exec_command'?'cmd':'command']:`printf '__LINK_BASH_${tag}__\\n'; ls .`,login:false,workdir:work,max_output_tokens:500})}:{type:'message',id:'msg_'+tag,role:'assistant',status:'completed',content:[{type:'output_text',text:'LINK-'+tag+'-COMPLETE',annotations:[],logprobs:[]}]};
   const events=[{type:'response.created',response:{id:'resp_'+tag,object:'response',status:'in_progress',output:[]}},{type:'response.output_item.added',output_index:0,item},{type:'response.output_item.done',output_index:0,item},{type:'response.completed',response:{id:'resp_'+tag,object:'response',status:'completed',output:[item],usage:{input_tokens:1,output_tokens:1,total_tokens:2}}}];
   res.writeHead(200,{'content-type':'text/event-stream'});res.end(events.map((e,i)=>`event: ${e.type}\ndata: ${JSON.stringify({...e,sequence_number:i})}\n\n`).join(''));
  });
 });
 await new Promise(r=>server.listen(0,'127.0.0.1',r));
 const callback=path.join(root,'original.cjs'),callbackLog=path.join(root,'original-events.jsonl');
 fs.writeFileSync(callback,"const fs=require('node:fs'),v=JSON.parse(process.argv.at(-1));fs.appendFileSync(process.argv[2],JSON.stringify({type:v.type,thread:v['thread-id'],turn:v['turn-id']})+'\\n');");
 const original=[process.execPath,callback,callbackLog];
 fs.writeFileSync(path.join(home,'config.toml'),`model='miniterm-link-fixture'
model_provider='fixture'
approval_policy='never'
sandbox_mode='danger-full-access'
cli_auth_credentials_store='file'
check_for_update_on_startup=false
web_search='disabled'
project_root_markers=[]
notify=${JSON.stringify(original)}
[tui]
screen_reader_detection_done=true
[tui.model_availability_nux]
"gpt-6.1-sol"=1
[model_providers.fixture]
name='Local shared notification fixture'
base_url='http://127.0.0.1:${server.address().port}/v1'
wire_api='responses'
requires_openai_auth=false
supports_websockets=false
request_max_retries=0
stream_max_retries=0
[projects.'${work.toLowerCase()}']
trust_level='trusted'
`);
 fs.writeFileSync(path.join(root,'settings.json'),JSON.stringify({schemaVersion:1,shortcut:'',notifications:false,profile:{kind:'git_bash',directory:work}}));
 const env={...process.env,CODEX_HOME:home,MINITERM_INSTANCE_ID:crypto.randomUUID(),MINITERM_DATA_DIR:root,MINITERM_DIAGNOSTICS:'1',TERM:'xterm-256color',COLORTERM:'truecolor'};
 for(const n of ['OPENAI_API_KEY','CODEX_API_KEY','OPENAI_BASE_URL','MINITERM_SESSION_ID','MINITERM_NOTIFY_EXE','MINITERM_AGENT_BASH','MINITERM_CODEX_SESSION_NOTIFY','CODEX_DAEMON_SHUTDOWN_SOCKET','CODEX_SESSION_ID','CODEX_THREAD_ID','NO_COLOR'])delete env[n];
 function invoke(exe,args,extra={}){const r=spawnSync(exe,args,{cwd:work,env:{...env,...extra},windowsHide:true,encoding:'utf8',timeout:30000});if(r.status!==0)throw new Error('Owned command failed: '+args[0]+': '+r.stderr);return r.stdout.trim();}
 const cli=(args,extra={})=>invoke(helper,args,extra),state=()=>JSON.parse(cli(['sessions']));
 async function until(predicate,label){const deadline=Date.now()+25000;while(Date.now()<deadline){if(await predicate())return;await pause(60);}throw new Error('Missing '+label);}
 const children=[];let gui;
 function start(name,args,extra={}){
  const terminal=new Terminal({cols:90,rows:35,allowProposedApi:true}),proc=pty.spawn(native,args,{cols:90,rows:35,cwd:work,env:{...env,...extra}});
  const child={name,proc,terminal,raw:'',chain:Promise.resolve(),exited:false};children.push(child);
  terminal.onData(d=>proc.write(d));proc.onData(d=>{child.raw+=d;child.chain=child.chain.then(()=>new Promise(r=>terminal.write(d,r)));});proc.onExit(()=>child.exited=true);return child;
 }
 async function screen(child){await child.chain;const b=child.terminal.buffer.active;return Array.from({length:child.terminal.rows},(_,r)=>b.getLine(b.viewportY+r)?.translateToString(true)||'').join('\n');}
 async function close(child){
  if(child.exited)return;
  await submit(child,'/quit').catch(()=>{});
  const deadline=Date.now()+2500;while(!child.exited&&Date.now()<deadline)await pause(30);
  // node-pty's console enumeration helper cannot attach to some native TUI
  // consoles. Stop only the native PID this fixture spawned as a fallback.
  if(!child.exited){try{process.kill(child.proc.pid);}catch{}}
  await until(()=>child.exited,'owned frontend exit');
 }
 async function linked(sid,args){const connection=cli(['codex-prepare','--',...args],{MINITERM_SESSION_ID:sid});const [url,token]=connection.split(' ');if(!url?.startsWith('ws://127.0.0.1:')||!token)throw new Error('Production helper did not create an authenticated link');return {connection,url,child:start('linked',['--no-alt-screen','--remote',url,'--remote-auth-token-env','MINITERM_CODEX_LINK_TOKEN','--cd',work,...args],{MINITERM_CODEX_LINK_TOKEN:token,MINITERM_SESSION_ID:sid}),sid};}
 async function detach(link){await close(link.child);cli(['codex-detach',link.connection],{MINITERM_SESSION_ID:link.sid});}
 const completed=tag=>requests.some(r=>(r.input||[]).some(i=>i.type==='function_call_output'&&i.call_id==='call_link_'+tag.toLowerCase()&&String(i.output).includes('__LINK_BASH_'+tag+'__')));
 function key(child,vk,scan,ch){for(const down of [1,0])child.proc.write('\x1b['+[vk,scan,ch,down,0,1].join(';')+'_');}
 async function submit(child,text){child.proc.write('\x1b[200~'+text+'\x1b[201~');await pause(200);key(child,13,28,13);}
 function records(){return fs.existsSync(callbackLog)?fs.readFileSync(callbackLog,'utf8').trim().split('\n').filter(Boolean).map(JSON.parse):[];}
 const result={root,nativeVersion,executableSha256:crypto.createHash('sha256').update(fs.readFileSync(helper)).digest('hex'),inputMode:'Background native ConPTY; no desktop mouse or keyboard synthesis',scope:'Production GUI IPC + installed Codex TUI, own daemon/config/callback/provider; no account or user configuration mutation'};
 try{
  cli(['integrate','codex','enable']);cli(['integrate','codex-bash','enable'],{PATH:path.dirname(native)+';'+env.PATH});
  gui=spawn(helper,[],{cwd:work,env,windowsHide:true,stdio:'ignore'});
  await until(()=>{try{return state().sessions.length===1;}catch{return false;}},'GUI IPC');
  const firstSid=state().sessions[0];cli(['diagnostic','new-session']);await until(()=>state().sessions.length===2,'second tab');
  const secondSid=state().sessions.find(id=>id!==firstSid);cli(['diagnostic','hide']);
  const first=await linked(firstSid,['LINK-FIRST']);await until(()=>completed('FIRST')&&state().details.states[firstSid]==='complete','first shared completion');
  let snapshot=state();fs.writeFileSync(path.join(root,'first-state.json'),JSON.stringify(snapshot,null,2));
  if(snapshot.details.states[secondSid]!=='unknown'||!snapshot.details.unread.includes(firstSid)||snapshot.details.noticeCount!==1)throw new Error('Shared completion was not isolated to the first tab');
  const {DatabaseSync}=require('node:sqlite'),db=new DatabaseSync(path.join(home,'state_5.sqlite'));let thread;try{const row=db.prepare('SELECT id,cwd FROM threads WHERE first_user_message=?').get('LINK-FIRST');if(!row)throw new Error('Main fixture conversation was not recorded');thread=row.id;const cwd=row.cwd.startsWith('\\\\?\\')?row.cwd.slice(4):row.cwd;if(path.resolve(cwd).toLowerCase()!==work.toLowerCase())throw new Error('Shared TUI changed the caller working directory');}finally{db.close();}
  await detach(first);
  const resumed=await linked(secondSid,['resume',thread,'LINK-RESUME']);await until(()=>completed('RESUME')&&state().details.states[secondSid]==='complete','resumed shared completion');
  snapshot=state();fs.writeFileSync(path.join(root,'resumed-state.json'),JSON.stringify(snapshot,null,2));
  if(snapshot.details.noticeCount!==2||snapshot.details.unread.length!==2)throw new Error('Resumed conversation was not assigned to its new tab');
  if(!requests.some(r=>{const text=JSON.stringify((r.input||[]).filter(i=>i.role==='user'));return text.includes('LINK-FIRST')&&text.includes('LINK-RESUME');}))throw new Error('Shared resume lost conversation history');
  // Keep this frontend connected so the conflicting private writer is real.
  const privateUi=start('private',['--no-alt-screen','--no-daemon','resume',thread],{MINITERM_SESSION_ID:secondSid});
  await until(async()=>/open in another app/.test(await screen(privateUi)),'real writer conflict');key(privateUi,82,19,114);await pause(800);
  if(!/open in another app/.test(await screen(privateUi)))throw new Error('Retry stole an existing writer');
  invoke(native,['app-server','daemon','stop']);key(privateUi,82,19,114);
  await until(async()=>!/open in another app/.test(await screen(privateUi)),'writer released');await pause(1600);await submit(privateUi,'LINK-RECOVERED');
  await until(()=>completed('RECOVERED'),'actual lowercase R recovery');await close(privateUi);await detach(resumed);
  const final=await linked(firstSid,['resume',thread,'LINK-FINAL']);await until(()=>completed('FINAL')&&state().details.states[firstSid]==='complete','final shared resume');
  await until(()=>records().filter(r=>r.thread===thread).length>=4,'original notification callback');
  const callbacks=records().filter(r=>r.thread===thread);if(callbacks.length!==4||new Set(callbacks.map(r=>r.turn)).size!==4)throw new Error('Original main-thread callback was duplicated or lost');
  cli(['diagnostic','quit']);await until(()=>gui.exitCode!==null,'GUI exit');
  const version=JSON.parse(invoke(native,['app-server','daemon','version']));if(version.status!=='running')throw new Error('GUI exit stopped the shared daemon');
  await close(final.child);
  const connectionClosed=await new Promise(resolve=>{const socket=net.createConnection(new URL(final.url).port,'127.0.0.1');socket.once('connect',()=>{socket.destroy();resolve(false);});socket.once('error',()=>resolve(true));});
  if(!connectionClosed)throw new Error('GUI exit leaked its listener');
  Object.assign(result,{passed:true,firstTabOnly:true,resumeMovedToSecondTab:true,historyPreserved:true,workingDirectoryPreserved:true,bashExecution:true,retryWhileWriterHeld:true,retryAfterWriterReleased:true,finalSharedResume:true,originalCallbackCount:callbacks.length,otherThreadCallbackCount:records().filter(r=>r.thread!==thread).length,daemonSurvivedGui:true,listenerClosed:true});
 }catch(error){result.passed=false;result.error=error.message;for(const c of children){await c.chain;fs.writeFileSync(path.join(root,c.name+'-screen.txt'),await screen(c));}try{fs.writeFileSync(path.join(root,'failure-state.json'),JSON.stringify(state(),null,2));}catch{};}
 finally{
  for(const c of children)await close(c).catch(()=>{});
  if(gui&&gui.exitCode===null){try{cli(['diagnostic','quit']);await until(()=>gui.exitCode!==null,'GUI cleanup');}catch{gui.kill();}}
  const stop=spawnSync(native,['app-server','daemon','stop'],{cwd:work,env,windowsHide:true,encoding:'utf8',timeout:15000});
  const cleanup=spawnSync('powershell.exe',['-NoProfile','-ExecutionPolicy','Bypass','-File',path.join(__dirname,'stop-fixture-updater.ps1'),'-Fixture',root],{windowsHide:true,encoding:'utf8',timeout:10000});
  result.ownedDaemonStopped=stop.status===0;result.updaterCleanup=cleanup.status===0;
  fs.writeFileSync(path.join(root,'updater-cleanup.json'),JSON.stringify({code:cleanup.status,stdout:cleanup.stdout,stderr:cleanup.stderr}));
  if(!result.ownedDaemonStopped||!result.updaterCleanup)result.passed=false;
  server.closeAllConnections();server.close();fs.writeFileSync(path.join(root,'requests.json'),JSON.stringify(requests,null,2));fs.writeFileSync(path.join(root,'verification.json'),JSON.stringify(result,null,2));
 }
 console.log(JSON.stringify(result));process.exit(result.passed?0:1);
})().catch(error=>{console.error(error);process.exitCode=1;});
