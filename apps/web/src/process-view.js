// Fold execution observations, never reinterpret a final answer merely because it is JSON.
export function processView(messages, running) {
  const rows=[]
  let process=null
  for(const row of messages){
    if(['user','message.created','context.reset'].includes(row.type)){if(process)process.pending=false;process=null}
    if(row.type==='tool'||row.type==='process'){
      if(!process){process={seq:`process-${row.seq}`,type:'process-group',timestamp:row.timestamp,details:[],pending:false};rows.push(process)}
      process.details.push(row)
      process.pending=process.details.some(item=>item.pending)
    }else{
      rows.push(row)
      if(['failed','agent.error','task.interrupted'].includes(row.type)||(row.type==='status'&&row.text==='已完成')){
        if(process)process.pending=false
        process=null
      }
    }
  }
  if(process)process.pending=running
  return rows
}
