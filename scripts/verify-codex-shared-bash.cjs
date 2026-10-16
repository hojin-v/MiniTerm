// Test persistent Bash configuration with the installed shared Codex backend.
// Every conversation, configuration, provider and daemon belongs to this fixture.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),http=require('node:http');
const {spawn,spawnSync}=require('node:child_process'),{createRequire}=require('node:module');
const dep=createRequire(path.join(process.env.APPDATA,'npm/node_modules/ai-battery/package.json'));
const pty=dep('node-pty'),{Terminal}=dep('@xterm/headless');
const pause=ms=>new Promise(r=>setTimeout(r,ms));
(async()=>{
 const root=path.resolve(__dirname,'../artifacts/shared-bash-'+crypto.randomUUID());
 const home=path.join(process.env.LOCALAPPDATA,'mt-b-'+crypto.randomBytes(4).toString('hex'),'h'),work=path.join(root,'work');
 fs.mkdirSync(home,{recursive:true});fs.mkdirSync(path.join(work,'.git'),{recursive:true});
 const exe=path.join(process.env.APPDATA,'npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe');
 const bash='C:/Program Files/Git/bin/bash.exe';if(!fs.existsSync(bash))throw new Error('Installed Git Bash required');
 fs.cpSync(path.join(process.env.USERPROFILE,'.codex/packages/app-server-daemon/releases/0.162.1-x86_64-pc-windows-msvc'),path.join(home,'packages/app-server-daemon/releases/0.162.1-x86_64-pc-windows-msvc'),{recursive:true});
 const migration=process.argv[3]==='migration';
 const win32Keyboard=process.argv[3]==='win32';
 const keyboard=process.argv[3]==='keyboard'||win32Keyboard;
 const requests=[];
 const server=http.createServer((req,res)=>{
  let body='';req.on('data',d=>body+=d);req.on('end',()=>{
   if(req.method!=='POST'||req.url!=='/v1/responses'){res.writeHead(404);res.end();return;}
   const request=JSON.parse(body);requests.push(request);fs.writeFileSync(path.join(root,'requests.json'),JSON.stringify(requests,null,2));
   const users=(request.input||[]).filter(i=>i.role==='user');
   const resumed=JSON.stringify(users.at(-1)).includes('SHARED-BASH-RESUME');
   const tag=resumed?'RESUME':'FIRST',call='call_bash_'+tag.toLowerCase();
   const toolOutput=(request.input||[]).some(i=>i.type==='function_call_output'&&i.call_id===call);
   const hasExec=(request.tools||[]).some(t=>t.name==='exec_command');
   const hasShell=(request.tools||[]).some(t=>t.name==='shell_command');
   let item;
   if((hasExec||hasShell)&&!toolOutput&&!(migration&&!resumed)){item={type:'function_call',id:'fc_'+tag,call_id:call,name:hasExec?'exec_command':'shell_command',arguments:JSON.stringify({[hasExec?'cmd':'command']:`task_label=$(printf '${tag}'); printf '__SHARED_BASH_%s__\\n' "$task_label"; printf '%s\\n' '*.txt' | sort; ls .`,login:false,workdir:work,max_output_tokens:1000})};}
   else {item={type:'message',id:'msg_'+tag,role:'assistant',status:'completed',content:[{type:'output_text',text:'SHARED-BASH-'+tag+'-COMPLETE',annotations:[],logprobs:[]}]};}
   const events=[{type:'response.created',response:{id:'resp_'+tag,object:'response',status:'in_progress',output:[]}},{type:'response.output_item.added',output_index:0,item}, {type:'response.output_item.done',output_index:0,item}, {type:'response.completed',response:{id:'resp_'+tag,object:'response',status:'completed',output:[item],usage:{input_tokens:1,output_tokens:1,total_tokens:2}}}];
   res.writeHead(200,{'content-type':'text/event-stream'});res.end(events.map((e,i)=>`event: ${e.type}\ndata: ${JSON.stringify({...e,sequence_number:i})}\n\n`).join(''));
  });
 });
 await new Promise(r=>server.listen(0,'127.0.0.1',r));
 const proxy=path.join(root,'exec-server-proxy.cjs');
 fs.writeFileSync(proxy, String.raw`const {spawn}=require('node:child_process'),path=require('node:path'),readline=require('node:readline');
const bash=process.argv[3];
const child=spawn(process.argv[2],['exec-server','--listen','stdio'],{env:{...process.env,PATH:path.dirname(bash)+';'+process.env.PATH},stdio:['pipe','pipe','inherit'],windowsHide:true});
process.stdin.pipe(child.stdin);
readline.createInterface({input:child.stdout}).on('line',line=>{
 const message=JSON.parse(line),result=message.result;
 const info=result?.environmentInfo||(result?.shell&&result?.platformOs?result:null);
 if(info){info.shell={name:'bash',path:bash};info.providerId='miniterm-git-bash:'+String(info.providerId||'native');}
 process.stdout.write(JSON.stringify(message)+'\n');
});
child.on('exit',c=>process.exit(c||0));`);
 fs.writeFileSync(path.join(home,'environments.toml'),`default='miniterm-git-bash'\ninclude_local=true\n[[environments]]\nid='miniterm-git-bash'\nprogram=${JSON.stringify(process.execPath)}\nargs=${JSON.stringify([proxy,exe,bash])}\n`);
 function install(){if(process.argv[2]){
  const helper=path.resolve(process.argv[2]);
  if(fs.existsSync(path.join(home,'environments.toml')))fs.unlinkSync(path.join(home,'environments.toml'));
  const registered=spawnSync(helper,['integrate','codex-bash','enable'],{env:{...process.env,CODEX_HOME:home,PATH:path.dirname(exe)+';'+process.env.PATH},windowsHide:true,encoding:'utf8'});
  if(registered.status!==0)throw new Error('Owned registration failed: '+registered.stderr);
 }};
 if(migration){fs.unlinkSync(path.join(home,'environments.toml'));}else{install();}
 const config=`model="miniterm-shared-bash-fixture"
model_provider="fixture"
approval_policy="never"
sandbox_mode="danger-full-access"
cli_auth_credentials_store="file"
web_search="disabled"
project_root_markers=[]
developer_instructions='Existing instructions are retained.'
[tui]
screen_reader_detection_done=true
[tui.model_availability_nux]
"gpt-6.1-sol"=1
[model_providers.fixture]
name="MiniTerm shared Bash fixture"
base_url="http://127.0.0.1:${server.address().port}/v1"
wire_api="responses"
requires_openai_auth=false
supports_websockets=false
request_max_retries=0
stream_max_retries=0
[projects.'${work.toLowerCase()}']
trust_level="trusted"
[projects.'${path.resolve(__dirname,'..').toLowerCase()}']
trust_level="trusted"
`;
 fs.writeFileSync(path.join(home,'config.toml'),config);
 fs.writeFileSync(path.join(root,'fixture-owned.json'),JSON.stringify({kind:'miniterm-local-shared-bash',root,home,work}));
 const env={...process.env,CODEX_HOME:home,TERM:'xterm-256color',COLORTERM:'truecolor'};
 for(const n of ['OPENAI_API_KEY','CODEX_API_KEY','OPENAI_BASE_URL','MINITERM_SESSION_ID','MINITERM_NOTIFY_EXE','MINITERM_AGENT_BASH','MINITERM_CODEX_SESSION_NOTIFY','CODEX_DAEMON_SHUTDOWN_SOCKET','CODEX_SESSION_ID','CODEX_THREAD_ID'])delete env[n];
 const children=[];
 function start(name,args){const terminal=new Terminal({cols:90,rows:35,allowProposedApi:true});const proc=pty.spawn(exe,args,{cols:90,rows:35,cwd:work,env});const c={name,proc,terminal,raw:'',chain:Promise.resolve(),exited:false};children.push(c);terminal.onData(d=>proc.write(d));proc.onData(d=>{c.raw+=d;c.chain=c.chain.then(()=>new Promise(r=>terminal.write(d,r)));});proc.onExit(()=>c.exited=true);return c;}
 async function until(predicate,label){const end=Date.now()+25000;while(Date.now()<end){if(predicate())return;await pause(25);}throw new Error('Missing '+label);}
 async function close(c){if(c.exited)return;try{process.kill(c.proc.pid);}catch{};await until(()=>c.exited,'owned CLI exit');}
 async function loadedThreads(){
  const proxy=spawn(exe,['app-server','proxy'],{cwd:work,env,windowsHide:true,stdio:['pipe','pipe','pipe']});
  // The control socket speaks WebSocket even through the raw stdio proxy.
  let buffer=Buffer.alloc(0),upgraded=false,next=1;const pending=new Map();
  const key=crypto.randomBytes(16).toString('base64');let readyResolve,readyReject;
  const ready=new Promise((resolve,reject)=>{readyResolve=resolve;readyReject=reject;});
  const readyTimer=setTimeout(()=>readyReject(new Error('Owned WebSocket upgrade timeout')),10000);
  function send(value,opcode=1){const payload=Buffer.isBuffer(value)?value:Buffer.from(JSON.stringify(value)),mask=crypto.randomBytes(4),header=Buffer.alloc(payload.length<126?6:8);header[0]=0x80|opcode;header[1]=0x80|(payload.length<126?payload.length:126);if(payload.length>=126)header.writeUInt16BE(payload.length,2);mask.copy(header,header.length-4);const masked=Buffer.from(payload);for(let i=0;i<masked.length;i++)masked[i]^=mask[i%4];proxy.stdin.write(Buffer.concat([header,masked]));}
  proxy.stdout.on('data',data=>{buffer=Buffer.concat([buffer,data]);if(!upgraded){const end=buffer.indexOf('\r\n\r\n');if(end<0)return;const header=buffer.subarray(0,end).toString();buffer=buffer.subarray(end+4);const accept=crypto.createHash('sha1').update(key+'258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64');if(!header.startsWith('HTTP/1.1 101')||!header.includes(accept)){clearTimeout(readyTimer);readyReject(new Error('Owned WebSocket upgrade rejected'));return;}upgraded=true;clearTimeout(readyTimer);readyResolve();}while(buffer.length>=2){let size=buffer[1]&127,offset=2;if(size===126){if(buffer.length<4)return;size=buffer.readUInt16BE(2);offset=4;}else if(size===127){if(buffer.length<10)return;size=Number(buffer.readBigUInt64BE(2));offset=10;}if(size>16*1024*1024){proxy.kill();return;}if(buffer.length<offset+size)return;const opcode=buffer[0]&15,payload=buffer.subarray(offset,offset+size);buffer=buffer.subarray(offset+size);if(opcode===9){send(payload,10);continue;}if(opcode!==1)continue;try{const m=JSON.parse(payload.toString()),p=pending.get(m.id);if(p){pending.delete(m.id);m.error?p.reject(new Error(JSON.stringify(m.error))):p.resolve(m.result);}}catch{}}});
  let stderr='';proxy.stderr.on('data',d=>stderr+=d);proxy.on('exit',()=>{clearTimeout(readyTimer);readyReject(new Error('Owned proxy exited: '+stderr));for(const p of pending.values())p.reject(new Error('Owned proxy exited: '+stderr));});
  proxy.stdin.write('GET / HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: '+key+'\r\nSec-WebSocket-Version: 13\r\n\r\n');
  async function call(method,params){return new Promise((resolve,reject)=>{const id=next++,timer=setTimeout(()=>{pending.delete(id);reject(new Error('Owned proxy timeout: '+method));},10000);pending.set(id,{resolve:r=>{clearTimeout(timer);resolve(r);},reject:e=>{clearTimeout(timer);reject(e);}});send({id,method,params});});}
  try{await ready;await call('initialize',{clientInfo:{name:'miniterm-owned-verification',version:'1'},capabilities:{experimentalApi:true}});send({method:'initialized'});return await call('thread/loaded/list',{limit:100});}finally{proxy.kill();}
 }
 const result={root,scope:'Installed Codex shared backend, account-free fixed SSE calls; no model or user configuration changes.'};
 const completed=tag=>requests.some(r=>(r.input||[]).some(i=>i.type==='function_call_output'&&i.call_id==='call_bash_'+tag.toLowerCase()&&String(i.output).includes('__SHARED_BASH_'+tag+'__')));
 try {
  const daemon=spawnSync(exe,['app-server','daemon','start'],{cwd:work,env,windowsHide:true,encoding:'utf8',timeout:20000});
  fs.writeFileSync(path.join(root,'daemon-start.json'),JSON.stringify({code:daemon.status,stdout:daemon.stdout,stderr:daemon.stderr}));
  if(daemon.status!==0)throw new Error('Owned shared daemon start failed: '+daemon.stderr);
  const first=start('first',keyboard?['--no-alt-screen']:['--no-alt-screen','SHARED-BASH-FIRST']);
  if(keyboard){
   await until(()=>first.raw.includes('OpenAI Codex'),'composer');await pause(3500);
   if(win32Keyboard){
    // Reproduce the console protocol separately from the GUI event loop.
    // These are Windows VK/scan identities for the fixed ASCII fixture input.
    if(!first.raw.includes('[?9001h'))throw new Error('Native console did not request Win32 input');
    const scans={S:31,H:35,A:30,R:19,E:18,D:32,B:48,F:33,I:23,T:20,'-':12};
    const key=(vk,scan,ch)=>`\x1b[${vk};${scan};${ch};1;0;1_\x1b[${vk};${scan};${ch};0;0;1_`;
    for(const ch of 'SHARED-BASH-FIRST'){first.proc.write(key(ch==='-'?189:ch.charCodeAt(0),scans[ch],ch.charCodeAt(0)));await pause(30);}
    await pause(600);first.proc.write(key(13,28,13));
   }else{first.proc.write('SHARED-BASH-FIRST');await pause(600);first.proc.write('\r');}
  }
  await until(()=>migration?requests.some(r=>(r.input||[]).some(i=>i.role==='user'&&JSON.stringify(i).includes('SHARED-BASH-FIRST'))):completed('FIRST'),'first actual turn');
  const {DatabaseSync}=require('node:sqlite'),db=new DatabaseSync(path.join(home,'state_5.sqlite'));
  let thread;try{thread=db.prepare('SELECT id FROM threads ORDER BY created_at DESC LIMIT 1').get().id;}finally{db.close();}
  result.thread=thread;await close(first);
  const loaded=await loadedThreads();result.sharedThreadLoaded=(loaded.data||[]).includes(thread);fs.writeFileSync(path.join(root,'loaded-threads.json'),JSON.stringify(loaded));
  if(migration){
   const stopped=spawnSync(exe,['app-server','daemon','stop'],{cwd:work,env,windowsHide:true,encoding:'utf8',timeout:15000});
   if(stopped.status!==0)throw new Error('Own migration daemon stop failed');
   install();
  }
  const resumed=start('resumed',['--no-alt-screen','resume',thread,'SHARED-BASH-RESUME']);await until(()=>completed('RESUME'),'resumed actual Bash output');
  result.migration=migration;result.keyboard=keyboard;result.win32Keyboard=win32Keyboard;result.firstBash=completed('FIRST');
  result.firstWasPowerShell=migration&&JSON.stringify(requests[0]).includes('<shell>powershell</shell>');
  result.resumedEnvironmentBash=requests.some(r=>JSON.stringify(r).includes('<shell>bash</shell>')&&JSON.stringify(r).includes('SHARED-BASH-RESUME')); result.resumedBash=completed('RESUME');
  result.guidancePreserved=requests.filter(r=>(r.tools||[]).some(t=>['exec_command','shell_command'].includes(t.name))).every(r=>JSON.stringify(r).includes('Existing instructions are retained.'));
  result.historyPreserved=requests.some(r=>{const users=JSON.stringify((r.input||[]).filter(i=>i.role==='user'));return users.includes('SHARED-BASH-FIRST')&&users.includes('SHARED-BASH-RESUME');});
  result.noWriterConflict=!children.some(c=>c.raw.includes('This conversation is open in another app'));
  // Codex may persist its own TUI notices; compare execution/config policy keys.
  const compare=spawnSync('python',['-c','import tomllib,sys,json; a=tomllib.loads(sys.stdin.read()); b=tomllib.load(open(sys.argv[1],"rb")); a.pop("tui",None); b.pop("tui",None); print(json.dumps(a)==json.dumps(b))',path.join(home,'config.toml')],{input:config,encoding:'utf8',windowsHide:true});
  result.configUnchanged=compare.status===0&&compare.stdout.trim()==='True';
 }catch(e){result.error=e.message;}
 finally {
  for(const c of children){await c.chain;fs.writeFileSync(path.join(root,c.name+'.vt'),c.raw);await close(c);c.terminal.dispose();}
  const stop=spawnSync(exe,['app-server','daemon','stop'],{cwd:work,env,windowsHide:true,encoding:'utf8',timeout:15000});result.ownedDaemonStopped=stop.status===0;
  const cleanup=spawnSync('powershell.exe',['-NoProfile','-ExecutionPolicy','Bypass','-File',path.join(__dirname,'stop-fixture-updater.ps1'),'-Fixture',root],{windowsHide:true,encoding:'utf8',timeout:10000});result.ownedUpdaterStopped=cleanup.status===0;fs.writeFileSync(path.join(root,'updater-cleanup.json'),JSON.stringify({code:cleanup.status,stdout:cleanup.stdout,stderr:cleanup.stderr}));
  result.passed=!result.error&&result.sharedThreadLoaded&&(migration?result.firstWasPowerShell:result.firstBash)&&result.resumedBash&&result.resumedEnvironmentBash&&result.guidancePreserved&&result.historyPreserved&&result.noWriterConflict&&result.configUnchanged&&result.ownedDaemonStopped&&result.ownedUpdaterStopped;
  fs.writeFileSync(path.join(root,'verification.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));server.closeAllConnections();server.close(()=>process.exit(result.passed?0:1));
 }
})().catch(e=>{console.error(e);process.exitCode=1;});
