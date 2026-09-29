export function processSummary(detail, running) {
  const name=detail.name||detail.label||'执行记录'
  if (/context|compact/.test(name)) return running&&detail.pending?'正在压缩上下文':'上下文处理完成'
  if(detail.type!=='tool') return /reason|think/.test(name)?'思考过程':'执行状态 · '+name
  let input=detail.input
  try { input=JSON.parse(input) } catch {}
  const command=input&&typeof input==='object'?input.command:undefined
  return `${running&&detail.pending?'正在运行':'已运行'} ${command||name}`.replace(/\s+/g,' ')
}

export function compactProcessSummary(detail) {
 if(detail.type!=='tool') return processSummary(detail,true)
 let input=detail.input
 try{input=JSON.parse(input)}catch{}
 const name=detail.name||detail.label||'tool'
 const command=typeof input?.command==='string'?input.command.trim().split(/\s+/)[0].replace(/^['"]|['"]$/g,'').split('/').pop():''
 const tool=name==='command_run'&&command?`command_run · ${command.slice(0,24)}`:name==='python_run'?'python':name
 return `${detail.pending?'执行中':'已完成'} · ${tool.replace(/[\x00-\x1f\x7f]/g,'').slice(0,40)}`
}
