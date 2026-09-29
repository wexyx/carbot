import test from 'node:test'
import assert from 'node:assert/strict'
import {spawn} from 'node:child_process'
import {once} from 'node:events'
import {mkdtemp,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join,resolve} from 'node:path'
import {pause,request,policy} from './admin-fixture.mjs'

test('CLI and Web share Agent/project/capability state; CLI works with Server stopped',{timeout:30000},async()=>{
 const dir=await mkdtemp(join(tmpdir(),'carbot-workbench-cli-'))
 const child=spawn(resolve('target/debug/agent-node'),['--cli','--data-dir',dir,'--server-port','0'],{env:{PATH:process.env.PATH,ADMIN_AGENT_PROVIDER:'mock',NODE_LINKS_JSON:'[]'},stdio:['pipe','pipe','pipe']})
 let output=''
 child.stdout.on('data',v=>output+=v);child.stderr.on('data',v=>output+=v)
 async function wait(predicate){for(let i=0;i<250;i++){if(predicate())return;if(child.exitCode!==null)throw Error(output);await pause(20)}throw Error(output)}
 async function command(line,marker){const offset=output.length;child.stdin.write(line+'\n');await wait(()=>output.slice(offset).includes(marker));return output.slice(offset)}
 try{
  await wait(()=>output.includes('group>'))
  const server={url:output.match(/Server: (http:\/\/\S+)/)[1]}
  const p=(await request(server,'/v1/repl')).projects[0].id,base='/v1/repl/'+p
  await command('/agent add parity mock CLI 创建','CLI 创建')
  let row=(await request(server,base+'/agents')).agents.find(a=>a.id==='parity')
  assert.equal(row.role,'CLI 创建')
  let response=await fetch(server.url+base+'/agents',{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify({client_id:'parity',provider:'mock',role:'Web 编辑',expected_version:row.version})})
  assert.equal(response.status,200,await response.text())
  assert.match(await command('/agent show parity','Web 编辑'),/Web 编辑/)
  await command('/agent start parity','started')
  await command('/project create '+JSON.stringify({name:'同一项目',policy:{...policy('parity'),mode:'chat'}}),'同一项目')
  const group=(await request(server,'/v1/repl')).collaboration_projects.find(g=>g.body.name==='同一项目')
  assert.ok(group)
  assert.match(await command('/project',group.key),/同一项目/)
  await command('/project '+group.key,group.key.slice(0,8))
  await command('/manage','admin')
  await command('/agent test parity','已进入 Agent 测试聊天')
  await command('测试共享会话','Mock')
  assert.ok(!(await request(server,'/v1/repl')).collaboration_projects.some(g=>g.body.kind==='agent_test'))
  await command('/manage','admin')
  assert.match(await command('/capabilities list {"scope":"business","kind":"tool"}','command_run'),/command_run/)
  assert.match(await command('/help network','/connect URL'),/人工|确认/)
  await command('/server stop','stopped')
  await command('/agent add offline mock 不依赖HTTP','不依赖HTTP')
  await command('/agent show offline','不依赖HTTP')
  await command('/agent delete offline 1 --confirm','deleted')
  const exit=once(child,'exit');child.stdin.end('/exit\n');await exit;assert.equal(child.exitCode,0)
 }finally{
  if(child.exitCode===null&&child.signalCode===null){const exit=once(child,'exit');child.kill('SIGTERM');await exit}
  await rm(dir,{recursive:true,force:true})
 }
})
