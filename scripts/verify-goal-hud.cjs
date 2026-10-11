// Account-free GUI integration: copy only a completed owned paused-goal seed.
const fs=require('node:fs');
const path=require('node:path');
const crypto=require('node:crypto');
const http=require('node:http');
const {spawn}=require('node:child_process');
const {DatabaseSync}=require('node:sqlite');
(async()=>{
  const artifacts=fs.realpathSync(path.resolve(__dirname,'../artifacts'));
  const seed=fs.realpathSync(process.argv[2]);
  const marker=JSON.parse(fs.readFileSync(path.join(seed,'fixture-owned.json'),'utf8'));
  const result=JSON.parse(fs.readFileSync(path.join(seed,'result.json'),'utf8'));
  if(path.dirname(seed)!==artifacts || !/^codex-ui-[0-9a-f-]{36}$/.test(path.basename(seed)) || marker.kind!=='miniterm-local-codex-ui' || marker.root!==seed || result.scenario!=='goal-seed' || result.error || !result.exited || fs.existsSync(path.join(seed,'codex-home/auth.json')))throw new Error('Expected a completed account-free local seed');
  const root=path.join(artifacts,'goal-hud-'+crypto.randomUUID());
  const home=path.join(root,'codex-home');
  fs.mkdirSync(root);
  fs.cpSync(path.join(seed,'codex-home'),home,{recursive:true});
  const db=new DatabaseSync(path.join(home,'state_5.sqlite'));
  try { db.prepare('UPDATE threads SET rollout_path=replace(rollout_path, ?, ?)').run(path.join(seed,'codex-home'),home); } finally { db.close(); }
  const work=path.join(seed,'work');
  fs.writeFileSync(path.join(root,'fixture-owned.json'),JSON.stringify({kind:'miniterm-local-goal-hud',root,home,work,seed}));
  fs.writeFileSync(path.join(root,'battery-fixture.cjs'),"process.stdout.write('AI Battery \\x1b[32m❚❚❚❚\\x1b[90m❚❚\\x1b[0m 67%');");
  let requests=0;
  const server=http.createServer((request,response)=>{
    request.on('data',()=>{});
    request.on('end',()=>{
      requests++;
      response.writeHead(400,{'content-type':'application/json'});
      response.end(JSON.stringify({error:{message:'MiniTerm local goal GUI fixture; no model executed.',type:'invalid_request_error',code:'fixture'}}));
    });
  });
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  fs.writeFileSync(path.join(home,'config.toml'),`model = "miniterm-ui-fixture"
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
[projects."${work.replaceAll('\\','/')}"]
trust_level = "trusted"
`);
  const child=spawn('powershell.exe',['-NoProfile','-ExecutionPolicy','Bypass','-File',path.join(__dirname,'verify-goal-hud.ps1'),'-Fixture',root],{stdio:['ignore','pipe','pipe'],windowsHide:true});
  let stdout='',stderr='';
  child.stdout.on('data',data=>{stdout+=data;});
  child.stderr.on('data',data=>{stderr+=data;});
  try {
    const code=await new Promise((resolve,reject)=>{child.once('exit',resolve);child.once('error',reject);});
    fs.writeFileSync(path.join(root,'harness-output.txt'),stdout+stderr);
    fs.writeFileSync(path.join(root,'provider.json'),JSON.stringify({requests,scope:'Local HTTP 400 only. No account, usage API or model requests.'},null,2));
    if(code!==0)throw new Error('GUI harness failed: '+stderr.trim());
    console.log(stdout.trim());
  } finally { server.close(); }
})().catch(error=>{console.error(error.message);process.exitCode=1;});
