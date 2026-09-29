export const MAX_ATTACHMENT_BYTES=10*1024*1024
export function splitAttachments(text){
  const files=[]
  const clean=text.replace(/\[附件：([^\]\r\n]*)\]\((\/v1\/attachments\/([0-9a-f-]{36})\/content)\)/g,(_,name,url,id)=>{
    if(!/^[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}$/.test(id))return _
    if(!files.some(f=>f.id===id))files.push({name,url,id})
    return ''
  })
  return {text:clean.trim(),files}
}
export function attachmentMessage(text,attachments){
  if(!attachments.length)return text
  return `${text.trim()||'请分析这些附件。'}\n\n${attachments.map(a=>a.reference).join('\n')}`
}
export function validateAttachment(file,count){
  if(count>=8)throw new Error('一次最多添加 8 个附件')
  if(file.size>MAX_ATTACHMENT_BYTES)throw new Error('附件不能超过 10 MiB')
  if(file.type.startsWith('image/')&&file.size>5*1024*1024)throw new Error('图片不能超过 5 MiB')
}

export function attachmentUrl(path,base){
 if(!/^\/v1\/attachments\/[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}\/content$/.test(path))throw new Error('Invalid attachment path')
 if(!base)return path
 const origin=new URL(base)
 if(!['http:','https:'].includes(origin.protocol)||origin.username||origin.password)throw new Error('Invalid Agent address')
 return base.replace(/\/$/,'')+path
}
