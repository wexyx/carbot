import test from 'node:test'
import {request as httpRequest} from 'node:http'
import assert from 'node:assert/strict'
import {mkdtemp,readFile,writeFile,readdir,rm,mkdir} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {start,stop,request,pause,modelFixture,act,policy,history} from './admin-fixture.mjs'
test('management is loopback-only and ignores obsolete admin tokens',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'carbot-auth-'));let server
  try{
    server=await start(dir,{ADMIN_TOKEN:'',ADMIN_AGENT_PROVIDER:'mock'})
    assert.equal((await fetch(server.url+'/v1/repl')).status,200)
    for(const headers of [{origin:'https://evil.example'},{host:'rebinding.example'},{'sec-fetch-site':'cross-site'}])assert.equal(await new Promise((resolve,reject)=>{const req=httpRequest(server.url+'/v1/repl',{headers},res=>{res.resume();resolve(res.statusCode)});req.on('error',reject);req.end()}),403,JSON.stringify(headers))
    assert.ok(!(await readdir(dir)).includes('admin-token'))
    await stop(server)
    server=await start(dir,{ADMIN_TOKEN:'secret-fixture',ADMIN_AGENT_PROVIDER:'mock'})
    assert.equal((await fetch(server.url+'/v1/repl')).status,200)
    assert.ok((await request(server,'/v1/repl')).projects.length)
  }finally{await stop(server);await rm(dir,{recursive:true,force:true})}
})
test('one admin chat appends JSONL, paginates, and migrates legacy snapshots with backup',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'carbot-jsonl-'));let server
  try{
    server=await start(dir,{ADMIN_AGENT_PROVIDER:'mock'})
    const p=(await request(server,'/v1/repl')).projects[0].id,base='/v1/admin-agent/'+p+'/sessions'
    const one=await request(server,base,{}),two=await request(server,base,{})
    assert.equal(one.id,two.id)
    for(const content of ['first','second']){
      await request(server,base+'/'+one.id+'/messages',{content})
      for(let i=0;i<100;i++){if((await request(server,base+'/'+one.id)).status!=='running')break;await pause(20)}
    }
    const logs=await request(server,'/v1/repl/'+p+'/chats/admin/logs')
    assert.deepEqual(logs.events.filter(e=>e.type==='user').map(e=>e.content),['first','second'])
    assert.equal(logs.files.length,1)
    const page=await request(server,'/v1/repl/'+p+'/chats/admin/logs?limit=2')
    const older=await request(server,'/v1/repl/'+p+'/chats/admin/logs?before='+page.events[0].seq)
    assert.equal(older.events.at(-1).seq+1,page.events[0].seq)
    const raw=await readFile(join(dir,'chats',p,Buffer.from('admin').toString('hex'),logs.files[0].name),'utf8')
    assert.deepEqual(raw.trim().split('\n').map(s=>JSON.parse(s)),logs.events)
    await stop(server)
    const state=JSON.parse(await readFile(join(dir,'state.json'),'utf8'))
    assert.equal(state.collections.management_sessions[p].events,undefined)
    assert.equal(state.collections.history,undefined)
    const legacy=join(dir,'legacy');await mkdir(legacy)
    state.collections.management_sessions[p].events=logs.events
    await writeFile(join(legacy,'state.json'),JSON.stringify(state))
    server=await start(legacy,{ADMIN_AGENT_PROVIDER:'mock'})
    const restored=await request(server,'/v1/repl/'+p+'/chats/admin/logs')
    assert.deepEqual(restored.events.filter(e=>e.type==='user').map(e=>e.content),['first','second'])
    assert.ok((await readdir(legacy)).includes('state.before-chat-jsonl.json'))
    await stop(server);server=await start(legacy,{ADMIN_AGENT_PROVIDER:'mock'})
    assert.equal((await request(server,'/v1/repl/'+p+'/chats/admin/logs')).events.length,logs.events.length)
  }finally{await stop(server);await rm(dir,{recursive:true,force:true})}
})
test('group ID owns all rounds without continuation links',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'carbot-group-log-')),model=await modelFixture();let server
  try{
    server=await start(dir,model.env)
    const p=(await request(server,'/v1/repl')).projects[0].id
    const result=await act(server,p,[{name:'agent_start',input:{client_id:'worker',role:'tester',provider:'mock'}},{name:'group_create',input:{name:'Chat',policy:policy('worker')}}])
    const group=result.outputs[1].key
    for(const content of ['hello round one','hello round two']){
      const run=await request(server,`/v1/repl/${p}/groups/${group}/messages`,{content})
      await history(server,p,run.id)
    }
    const log=await request(server,`/v1/repl/${p}/chats/${group}/logs`)
    assert.deepEqual(log.events.filter(e=>e.type==='message.created').map(e=>e.content),['hello round one','hello round two'])
    assert.equal(log.files.length,1);assert.equal(log.active_run,null)
  }finally{await stop(server);await model.close();await rm(dir,{recursive:true,force:true})}
})
