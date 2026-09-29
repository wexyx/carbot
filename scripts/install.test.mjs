import test from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp,mkdir,writeFile,readFile,rm,readdir} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join,resolve} from 'node:path'
import {execFileSync,spawnSync} from 'node:child_process'
import {createHash} from 'node:crypto'

const installer=resolve('install.sh')
async function fixture(run){
  const root=await mkdtemp(join(tmpdir(),'carbot-install-'))
  try{
    const home=join(root,'home'),assets=join(root,'assets'),bin=join(root,'bin'),bundle=join(root,'bundle/carbot')
    for(const path of [home,assets,bin,join(bundle,'bin'),join(bundle,'libexec'),join(bundle,'web'),join(bundle,'skills/system/management/management-guide')])await mkdir(path,{recursive:true})
    for(const path of ['bin/carbot','libexec/agent-node'])await writeFile(join(bundle,path),'#!/bin/sh\nprintf "installed-fixture\\n"\n',{mode:0o755})
    await writeFile(join(bundle,'web/index.html'),'fixture')
    await writeFile(join(bundle,'skills/system/management/management-guide/SKILL.md'),'fixture')
    const os=execFileSync('uname',['-s'],{encoding:'utf8'}).trim(),arch=execFileSync('uname',['-m'],{encoding:'utf8'}).trim()
    const target=({'Darwin:arm64':'aarch64-apple-darwin','Darwin:x86_64':'x86_64-apple-darwin','Linux:x86_64':'x86_64-unknown-linux-gnu','Linux:aarch64':'aarch64-unknown-linux-gnu','Linux:arm64':'aarch64-unknown-linux-gnu'})[os+':'+arch]
    assert.ok(target,'Unsupported test platform')
    const archive=join(assets,`carbot-${target}.tar.gz`)
    execFileSync('tar',['-czf',archive,'-C',join(root,'bundle'),'carbot'])
    await writeFile(archive+'.sha256',createHash('sha256').update(await readFile(archive)).digest('hex')+'  fixture\n')
    // Offline release transport; still exercises real archive checks, checksum and installation.
    await writeFile(join(bin,'curl'),`#!${process.execPath}
const fs=require('node:fs'),path=require('node:path'),args=process.argv.slice(2);
const url=args.find(v=>v.startsWith('https://')),out=args[args.indexOf('-o')+1];
fs.copyFileSync(path.join(process.env.FIXTURE_ASSETS,path.basename(url)),out);
`,{mode:0o755})
    const prefix=join(home,"custom path '$install")
    const env={...process.env,HOME:home,CARBOT_INSTALL_PREFIX:prefix,CARBOT_VERSION:'v0.0.0',FIXTURE_ASSETS:assets,PATH:bin+':/usr/bin:/bin:/usr/sbin:/sbin'}
    delete env.ZDOTDIR;delete env.XDG_CONFIG_HOME
    await run({root,home,prefix,env,archive})
  }finally{await rm(root,{recursive:true,force:true})}
}

test('installer configures Bash command discovery, preserves profiles and is idempotent',async()=>fixture(async({home,prefix,env})=>{
  const profile=join(home,'.bashrc'),original='# user settings\nexport USER_SETTING=preserved\n'
  await writeFile(profile,original)
  for(let i=0;i<2;i++){
    const installed=spawnSync('bash',[installer],{env:{...env,SHELL:'/bin/bash'},encoding:'utf8'})
    assert.equal(installed.status,0,installed.stderr)
    const result=spawnSync('/bin/bash',['--noprofile','--norc','-c','. "$HOME/.profile"; . "$HOME/.bashrc"; command -v carbot; carbot; printf "%s" "$USER_SETTING"'],{env,encoding:'utf8'})
    assert.equal(result.status,0,result.stderr)
    assert.ok(result.stdout.includes(prefix+'/bin/carbot'))
    assert.ok(result.stdout.includes('installed-fixture'))
    assert.ok(result.stdout.endsWith('preserved'))
  }
  const content=await readFile(profile,'utf8')
  assert.ok(content.startsWith(original))
  assert.equal(content.split('# Carbot PATH').length-1,1)
  const backups=(await readdir(home)).filter(p=>p.startsWith('.bashrc.carbot-backup.'))
  assert.equal(backups.length,1)
  assert.equal(await readFile(join(home,backups[0]),'utf8'),original)
}))

test('installer writes zsh and fish startup configuration without replacing user settings',async()=>fixture(async({home,env})=>{
  for(const shell of ['zsh','fish']){
    const installed=spawnSync('bash',[installer],{env:{...env,SHELL:'/bin/'+shell},encoding:'utf8'})
    assert.equal(installed.status,0,installed.stderr)
    const file=shell==='zsh'?join(home,'.zshrc'):join(home,'.config/fish/conf.d/carbot.fish')
    assert.match(await readFile(file,'utf8'),/# Carbot PATH/)
    if(shell==='zsh' && process.platform==='darwin'){
      const result=spawnSync('/bin/zsh',['-f','-c','source "$HOME/.zshrc"; carbot'],{env,encoding:'utf8'})
      assert.equal(result.status,0,result.stderr)
      assert.match(result.stdout,/installed-fixture/)
    }
  }
}))

test('checksum failure does not create a command or change shell configuration',async()=>fixture(async({home,env,archive})=>{
  await writeFile(archive+'.sha256','0'.repeat(64)+'  fixture\n')
  const result=spawnSync('bash',[installer],{env:{...env,SHELL:'/bin/bash'},encoding:'utf8'})
  assert.notEqual(result.status,0)
  assert.match(result.stderr,/Checksum mismatch/)
  assert.deepEqual(await readdir(home),[])
}))
