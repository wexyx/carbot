import {spawn} from 'node:child_process'
import {mkdir,readFile} from 'node:fs/promises'
import {fileURLToPath} from 'node:url'
import {dirname,join} from 'node:path'

const runtime=join(dirname(fileURLToPath(import.meta.url)),'.runtime')
await mkdir(runtime,{recursive:true})
const env={...process.env,PUPPETEER_CACHE_DIR:join(runtime,'browsers'),PUPPETEER_SKIP_DOWNLOAD:'true'}
async function run(binary,args){
 await new Promise((resolve,reject)=>{const child=spawn(binary,args,{cwd:runtime,env,stdio:'inherit'});child.once('error',reject);child.once('exit',code=>code===0?resolve():reject(Error(`${binary} exited ${code}`)))})
}
await run('npm',['install','--prefix',runtime,'--ignore-scripts','--no-audit','--no-fund','--save-exact','puppeteer@25.12.0'])
delete env.PUPPETEER_SKIP_DOWNLOAD
const packageDir=join(runtime,'node_modules/puppeteer')
const manifest=JSON.parse(await readFile(join(packageDir,'package.json'),'utf8'))
await run(process.execPath,[join(packageDir,typeof manifest.bin==='string'?manifest.bin:manifest.bin.puppeteer),'browsers','install','chrome-headless-shell'])
console.log('Puppeteer installed. Run: node browser.mjs https://example.com screenshot.png')
