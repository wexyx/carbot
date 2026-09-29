export function processSummary(detail, running) {
  const name=detail.name||detail.label||'执行记录'
  if (/context|compact/.test(name)) return running&&detail.pending?'正在压缩上下文':'上下文处理完成'
  if(detail.type!=='tool') return /reason|think/.test(name)?'思考过程':'执行状态 · '+name
  let input=detail.input
  try { input=JSON.parse(input) } catch {}
  const command=input&&typeof input==='object'?input.command:undefined
  return `${running&&detail.pending?'正在运行':'已运行'} ${command||name}`.replace(/\s+/g,' ')
}
