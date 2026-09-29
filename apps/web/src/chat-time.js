export function eventTime(event) {
  const raw=event.logged_at??event.timestamp??event.created_at
  if(raw===undefined||raw===null||raw==='')return null
  const numeric=typeof raw==='number'||/^\d+(\.\d+)?$/.test(String(raw))
  const value=numeric?Number(raw):raw
  const date=new Date(numeric&&value<1e12?value*1000:value)
  return Number.isFinite(date.getTime())?date.toISOString():null
}
export function messageClock(value){return value?new Date(value).toLocaleTimeString('zh-CN',{hour:'2-digit',minute:'2-digit',hour12:false}):''}
export function messageDate(value){return value?new Date(value).toLocaleDateString('zh-CN',{year:'numeric',month:'2-digit',day:'2-digit'}):''}
