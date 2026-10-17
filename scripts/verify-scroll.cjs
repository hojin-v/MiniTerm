// Real MiniTerm/Codex GUI with an account-free localhost SSE provider.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const http = require('node:http');
const {spawn,spawnSync} = require('node:child_process');
(async () => {
  const root = path.resolve(__dirname, '../artifacts/scroll-' + crypto.randomUUID());
  const shared=process.argv[4]==='shared';
  const streaming=process.env.MINITERM_SCROLL_STREAMING==='1';
  const notification=shared&&process.env.MINITERM_SCROLL_NOTIFY==='1';
  const home = shared?path.join(process.env.LOCALAPPDATA,'mt-s-'+crypto.randomBytes(4).toString('hex'),'h'):path.join(root, 'codex-home'), work = path.join(root, 'work');
  fs.mkdirSync(home, {recursive:true}); fs.mkdirSync(work,{recursive:true});
  const native=path.join(process.env.APPDATA,'npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe');
  const versionResult=spawnSync(native,['--version'],{windowsHide:true,encoding:'utf8',timeout:10000});
  const nativeVersion=/^codex-cli (\d+\.\d+\.\d+)/.exec(versionResult.stdout||'')?.[1];
  if(versionResult.status!==0||!nativeVersion)throw new Error('Cannot identify the installed Codex version.');
  if(shared){const release=nativeVersion+'-x86_64-pc-windows-msvc';fs.cpSync(path.join(process.env.USERPROFILE,'.codex/packages/app-server-daemon/releases',release),path.join(home,'packages/app-server-daemon/releases',release),{recursive:true});}
  // Establish the fixture's own project root, so trusting it cannot target the repository.
  fs.mkdirSync(path.join(work, '.git'));
  const requests = [];
  const server = http.createServer((request, response) => {
    let body = '';
    request.on('data', data => { body += data; });
    request.on('end', () => {
      if (request.url !== '/v1/responses' || request.method !== 'POST') {
        response.writeHead(404); response.end(); return;
      }
      const payload=JSON.parse(body);
      requests.push(payload);
      fs.writeFileSync(path.join(root, 'requests.json'), JSON.stringify(requests, null, 2));
      const firstTurn=((payload.input||[]).filter(i=>i.role==='user').at(-1)?.content||[]).some(p=>p.text==='SCROLL-FIRST-TURN');
      const cancelTurn=((payload.input||[]).filter(i=>i.role==='user').at(-1)?.content||[]).some(p=>p.text==='SCROLL-CANCEL-TURN');
      const toolOutput=(payload.input||[]).some(i=>i.type==='function_call_output'&&i.call_id==='call_scroll_environment');
      const text = firstTurn
        ? Array.from({length:180}, (_, i) => `SCROLL-LINE-${String(i).padStart(3,'0')} fixed local output`).join('\n')
        : 'SCROLL-DRAFT-ACCEPTED';
      const part = {type:'output_text', text, annotations:[], logprobs:[]};
      const item = {id:'msg_scroll', type:'message', role:'assistant', status:'completed', content:[part]};
      if(shared&&firstTurn&&!toolOutput){
        const tool=(payload.tools||[]).find(t=>t.name==='exec_command'||t.name==='shell_command');
        if(!tool){response.writeHead(400,{'content-type':'application/json'});response.end(JSON.stringify({error:{message:'Owned Bash execution tool is missing.'}}));return;}
        const call={type:'function_call',id:'fc_scroll_environment',call_id:'call_scroll_environment',name:tool.name,arguments:JSON.stringify({[tool.name==='exec_command'?'cmd':'command']:'printf "__SCROLL_CALLER_%s__\\n" "$MINITERM_SCROLL_CALLER"; ls .',login:false,workdir:work,max_output_tokens:1000})};
        const events=[{type:'response.created',response:{id:'resp_scroll_tool',object:'response',status:'in_progress',output:[]}},{type:'response.output_item.added',output_index:0,item:call},{type:'response.output_item.done',output_index:0,item:call},{type:'response.completed',response:{id:'resp_scroll_tool',object:'response',status:'completed',output:[call],usage:{input_tokens:1,output_tokens:1,total_tokens:2}}}];
        response.writeHead(200,{'content-type':'text/event-stream'});
        response.end(events.map((e,i)=>`event: ${e.type}\ndata: ${JSON.stringify({...e,sequence_number:i})}\n\n`).join(''));
        return;
      }
      const events = [
        {type:'response.created', response:{id:'resp_scroll', object:'response', model:'miniterm-scroll-fixture', status:'in_progress', output:[]}},
        {type:'response.output_item.added', output_index:0, item:{...item, status:'in_progress', content:[]}},
        {type:'response.content_part.added', output_index:0, content_index:0, item_id:item.id, part:{...part,text:''}},
        {type:'response.output_text.delta', output_index:0, content_index:0, item_id:item.id, delta:text},
        {type:'response.output_text.done', output_index:0, content_index:0, item_id:item.id, text},
        {type:'response.content_part.done', output_index:0, content_index:0, item_id:item.id, part},
        {type:'response.output_item.done', output_index:0, item},
        {type:'response.completed', response:{id:'resp_scroll', object:'response', model:'miniterm-scroll-fixture', status:'completed', output:[item], usage:{input_tokens:1,output_tokens:1,total_tokens:2}}}
      ];
      const finish=()=>{
        response.writeHead(200, {'content-type':'text/event-stream'});
        if(cancelTurn){
          response.write(events.slice(0,3).map((event,i)=>`event: ${event.type}\ndata: ${JSON.stringify({...event,sequence_number:i})}\n\n`).join(''));
          fs.writeFileSync(path.join(root,'cancel-waiting.json'),JSON.stringify({waiting:true}));
          response.once('close',()=>fs.writeFileSync(path.join(root,'cancel-closed.json'),JSON.stringify({closed:true})));
          return;
        }
        if(streaming&&firstTurn){
          let sequence=0,chunk=0;
          const send=event=>response.write(`event: ${event.type}\ndata: ${JSON.stringify({...event,sequence_number:sequence++})}\n\n`);
          events.slice(0,3).forEach(send);
          const chunks=text.match(/(?:[^\n]*\n?){1,3}/g).filter(Boolean);
          const timer=setInterval(()=>{
            send({...events[3],delta:chunks[chunk++]});
            fs.writeFileSync(path.join(root,'stream-progress.json'),JSON.stringify({chunk,total:chunks.length}));
            if(chunk===chunks.length){clearInterval(timer);events.slice(4).forEach(send);response.end();fs.writeFileSync(path.join(root,'first-turn-completed.json'),JSON.stringify({toolOutput,streaming:true}));}
          },250);
          response.once('close',()=>clearInterval(timer));
          return;
        }
        response.end(events.map((event,i) => `event: ${event.type}\ndata: ${JSON.stringify({...event, sequence_number:i})}\n\n`).join(''));
        if(firstTurn)fs.writeFileSync(path.join(root,'first-turn-completed.json'),JSON.stringify({toolOutput}));
      };
      // Let the GUI select its second tab before the owned first turn completes.
      if(notification&&firstTurn)setTimeout(finish,2000);else finish();
    });
  });
  await new Promise(resolve => server.listen(0,'127.0.0.1',resolve));
  fs.writeFileSync(path.join(home,'config.toml'), `model = "miniterm-scroll-fixture"
model_provider = "fixture"
approval_policy = "never"
sandbox_mode = "${process.argv[5]==='full'?'danger-full-access':'read-only'}"
cli_auth_credentials_store = "file"
web_search = "disabled"
project_root_markers = []
check_for_update_on_startup = false
[tui]
screen_reader_detection_done = true
[tui.model_availability_nux]
"gpt-6.1-sol" = 1
[model_providers.fixture]
name = "MiniTerm Scroll Fixture"
base_url = "http://127.0.0.1:${server.address().port}/v1"
wire_api = "responses"
requires_openai_auth = false
supports_websockets = false
request_max_retries = 0
stream_max_retries = 0
[projects.'${work.toLowerCase()}']
trust_level = "trusted"
`);
  fs.writeFileSync(path.join(root,'fixture-owned.json'), JSON.stringify({kind:'miniterm-local-scroll',root,home,work}));
  fs.writeFileSync(path.join(root,'battery-fixture.cjs'), "process.stdout.write('AI Battery \\x1b[32m❚❚❚❚\\x1b[90m❚❚\\x1b[0m 67%');");
  const args = ['-NoProfile','-ExecutionPolicy','Bypass','-File',path.join(__dirname,'verify-scroll.ps1'),'-Fixture',root];
  if (process.argv[2] && process.argv[2]!=='default') args.push('-BatteryLayout', process.argv[2]);
  if (process.argv[3]) args.push('-Executable', process.argv[3]);
  const retry=shared&&process.env.MINITERM_SCROLL_RETRY==='1';
  if(retry)args.push('-Retry');
  if(notification)args.push('-CodexNotify');
  if(shared){
    args.push('-Shared');
    const helper=path.resolve(__dirname,'..',process.env.MINITERM_SCROLL_EXECUTOR_HELPER||process.argv[3]||'artifacts/miniterm.exe');
    const registered=spawnSync(helper,['integrate','codex-bash','enable'],{env:{...process.env,CODEX_HOME:home,MINITERM_DATA_DIR:root,PATH:path.dirname(native)+';'+process.env.PATH},encoding:'utf8',windowsHide:true});
    if(registered.status!==0)throw new Error('Owned Bash registration failed: '+registered.stderr);
    if(notification||process.env.MINITERM_SCROLL_LINK==='1'){
      const notify=spawnSync(helper,['integrate','codex','enable'],{env:{...process.env,CODEX_HOME:home,MINITERM_DATA_DIR:root},encoding:'utf8',windowsHide:true});
      if(notify.status!==0)throw new Error('Owned notification registration failed: '+notify.stderr);
    }
    if(process.env.MINITERM_SCROLL_PRESTART_DAEMON==='1'){
      const daemonEnv={...process.env,CODEX_HOME:home};
      for(const key of ['CODEX_DAEMON_SHUTDOWN_SOCKET','CODEX_SESSION_ID','CODEX_THREAD_ID'])delete daemonEnv[key];
      const started=spawnSync(native,['app-server','daemon','start'],{env:daemonEnv,cwd:work,encoding:'utf8',windowsHide:true,timeout:20000});
      fs.writeFileSync(path.join(root,'daemon-prestart.json'),JSON.stringify({code:started.status,stdout:started.stdout,stderr:started.stderr}));
      if(started.status!==0)throw new Error('Owned daemon prestart failed: '+started.stderr);
    }
  }
  const child = spawn('powershell.exe', args, {windowsHide:true,stdio:['ignore','pipe','pipe']});
  let output = '';
  child.stdout.on('data',data=>{ output+=data; }); child.stderr.on('data',data=>{ output+=data; });
  try {
    const code = await new Promise((resolve,reject)=>{child.once('exit',resolve);child.once('error',reject);});
    fs.writeFileSync(path.join(root,'harness-output.txt'),output);
    if(shared&&code===0){
      const daemonEnv={...process.env,CODEX_HOME:home};
      for(const key of ['CODEX_DAEMON_SHUTDOWN_SOCKET','CODEX_SESSION_ID','CODEX_THREAD_ID'])delete daemonEnv[key];
      // This queries the running control socket and does not start a daemon.
      // The installed CLI has no `status` subcommand; a stopped owned daemon
      // rejects `version` with a connection error.
      const status=spawnSync(native,['app-server','daemon','version'],{env:daemonEnv,cwd:work,encoding:'utf8',windowsHide:true,timeout:10000});
      fs.writeFileSync(path.join(root,'daemon-after-gui.json'),JSON.stringify({code:status.status,stdout:status.stdout,stderr:status.stderr}));
      if(status.status!==0||JSON.parse(status.stdout).status!=='running')throw new Error('Owned managed server did not survive GUI close.');
    }
    if(process.argv[2]==='probe'){console.log(JSON.stringify({root,code,output}));process.exitCode=code;return;}
    const userTexts = requests.flatMap(r=>(r.input||[]).filter(i=>i.role==='user').flatMap(i=>(i.content||[]).filter(p=>p.type==='input_text').map(p=>p.text)));
    // Check the first request too: an intact second draft cannot prove that
    // startup OSC replies were consumed instead of being inserted as text.
    const firstPromptClean = requests.length > 0 && (requests[0].input||[]).some(i=>i.role==='user' && (i.content||[]).some(p=>p.type==='input_text' && p.text==='SCROLL-FIRST-TURN'));
    const draftPreserved = userTexts.includes('SCROLL-DRAFT-KEEP-TAIL');
    const cancelledStreamRecovered=!streaming||(fs.existsSync(path.join(root,'cancel-closed.json'))&&userTexts.includes('SCROLL-AFTER-CANCEL'));
    const resumeHistoryPreserved=!retry||requests.some(r=>{const text=JSON.stringify((r.input||[]).filter(i=>i.role==='user'));return text.includes('SCROLL-FIRST-TURN')&&text.includes('SCROLL-DRAFT-KEEP-TAIL')&&text.includes('SCROLL-RESUME-HISTORY');});
    const retryInputClean=!retry||userTexts.includes('SCROLL-RETRY-RECOVERED');
    const finalSharedInput=!retry||userTexts.includes('SCROLL-FINAL-SHARED');
    const callerEnvironmentPreserved=!shared||requests.some(r=>(r.input||[]).some(i=>i.type==='function_call_output'&&i.call_id==='call_scroll_environment'&&String(i.output).includes('__SCROLL_CALLER_OWNED-ENV__')));
    const passed=firstPromptClean&&draftPreserved&&cancelledStreamRecovered&&resumeHistoryPreserved&&retryInputClean&&finalSharedInput&&callerEnvironmentPreserved;
    if (code === 0 && fs.existsSync(path.join(root,'verification.json'))) {
      const result = JSON.parse(fs.readFileSync(path.join(root,'verification.json'),'utf8'));
      fs.writeFileSync(path.join(root,'verification.json'),JSON.stringify({...result,nativeVersion,firstPromptClean,draftPreserved,cancelledStreamRecovered,managedServerSurvived:shared,resumeHistoryPreserved,retryInputClean,finalSharedInput,callerEnvironmentPreserved,passed},null,2));
    }
    fs.writeFileSync(path.join(root,'provider.json'),JSON.stringify({requests:requests.length,firstPromptClean,draftPreserved,scope:'Only own fixed localhost Responses fixture; no credentials or model.'},null,2));
    console.log(JSON.stringify({root,code,firstPromptClean,draftPreserved,output:output.trim()}));
    process.exitCode = code || (passed?0:1);
  } finally {
    if(shared){const stopped=spawnSync(native,['app-server','daemon','stop'],{env:{...process.env,CODEX_HOME:home},cwd:work,encoding:'utf8',windowsHide:true,timeout:15000});fs.writeFileSync(path.join(root,'owned-daemon-stop.json'),JSON.stringify({code:stopped.status}));const cleanup=spawnSync('powershell.exe',['-NoProfile','-ExecutionPolicy','Bypass','-File',path.join(__dirname,'stop-fixture-updater.ps1'),'-Fixture',root],{encoding:'utf8',windowsHide:true,timeout:10000});fs.writeFileSync(path.join(root,'updater-cleanup.json'),JSON.stringify({code:cleanup.status,stdout:cleanup.stdout,stderr:cleanup.stderr}));if(cleanup.status!==0)process.exitCode=1;}
    server.closeAllConnections();server.close();
  }
})().catch(error=>{console.error(error);process.exitCode=1;});
