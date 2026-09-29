import {createRequire} from 'node:module'
import {join} from 'node:path'
import {pathToFileURL} from 'node:url'
const input=JSON.parse(process.argv[1])
process.env.PUPPETEER_CACHE_DIR=join(input.runtime,'browsers')
const require=createRequire(join(input.runtime,'package.json'))
const {default:puppeteer}=await import(pathToFileURL(require.resolve('puppeteer')).href)
const {launch,CDP_WEBSOCKET_ENDPOINT_REGEX}=await import(pathToFileURL(require.resolve('@puppeteer/browsers')).href)
let browser,processHandle
try{
 const options={headless:'shell',userDataDir:input.profile,args:['--disable-gpu']}
 // Keep Chromium in the host-owned process group so interruption kills all children.
 processHandle=launch({executablePath:await puppeteer.executablePath(options),args:[...await puppeteer.defaultArgs(options),'--remote-debugging-port=0'],detached:false,env:process.env})
 browser=await puppeteer.connect({browserWSEndpoint:await processHandle.waitForLineOutput(CDP_WEBSOCKET_ENDPOINT_REGEX,30000)})
 const page=await browser.newPage()
 page.setDefaultTimeout(30000)
 page.setDefaultNavigationTimeout(60000)
 await page.goto(input.url,{waitUntil:'domcontentloaded'})
 if(input.screenshot)await page.screenshot({path:input.screenshot,fullPage:true})
 const text=await page.evaluate(()=>document.body?.innerText||'')
 console.log(JSON.stringify({url:page.url(),title:await page.title(),text:text.slice(0,12000),truncated:text.length>12000,screenshot:input.screenshot}))
}finally{try{await browser?.close()}finally{await processHandle?.close()}}
