const bytes=text=>new TextEncoder().encode(text).length
const validPath=path=>path.length<=200&&path.split('/').every(p=>p&&!p.startsWith('.')&&/^[a-zA-Z0-9_.-]+$/.test(p))
export function parseToolImport(text){
 if(bytes(text)>600000)throw new Error('工具定义文件过大。')
 const input=JSON.parse(text),items=Array.isArray(input)?input:[input]
 if(!items.length||items.length>64)throw new Error('一次导入 1–64 个外部命令。')
 const names=new Set()
 return items.map(item=>{
  if(!item||Object.keys(item).some(k=>!['name','description','command','enabled'].includes(k)))throw new Error('仅支持外部命令定义：name、description、command、enabled。')
  if(typeof item.name!=='string'||!/^[a-zA-Z0-9_]{1,64}$/.test(item.name)||names.has(item.name))throw new Error('工具名称无效或重复。')
  if(typeof item.description!=='string'||!item.description.trim()||bytes(item.description)>2048)throw new Error('用途描述不能为空，最多 2048 字节。')
  if(typeof item.command!=='string'||!item.command.trim()||bytes(item.command)>8192||/[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]/.test(item.command))throw new Error('命令为空、过长或包含控制字符。')
  names.add(item.name)
  return {...item,enabled:false}
 })
}
export async function parseSkillImport(files){
 if(!files.length||files.length>32)throw new Error('请选择 1–32 个 UTF-8 文本文件。')
 if(files.reduce((sum,f)=>sum+f.size,0)>600000)throw new Error('导入文件过大。')
 const read=async file=>new TextDecoder('utf-8',{fatal:true}).decode(await file.arrayBuffer())
 let definition
 if(files.length===1&&files[0].name.endsWith('.json')){
  definition=JSON.parse(await read(files[0]))
  if(!definition||Object.keys(definition).some(k=>!['id','description','files','enabled','allow_python','execution_profile'].includes(k)))throw new Error('无效的 Skill JSON 定义。')
 }else{
  const root=files[0].webkitRelativePath?.split('/')[0]||'imported-skill'
  const entries={}
  for(const file of files){
   const name=file.webkitRelativePath?file.webkitRelativePath.split('/').slice(1).join('/'):file.name
   if(!validPath(name)||Object.hasOwn(entries,name))throw new Error('文件路径无效或重复：'+name)
   entries[name]=await read(file)
  }
  definition={id:root.replace(/[^a-zA-Z0-9_-]/g,'-').slice(0,64)||'imported-skill',description:'导入的 Skill，请补充用途描述',files:entries}
 }
 if(typeof definition.id!=='string'||!/^[a-zA-Z0-9_-]{1,64}$/.test(definition.id))throw new Error('Skill ID 无效。')
 if(typeof definition.description!=='string'||!definition.description.trim()||bytes(definition.description)>2048)throw new Error('Skill 描述无效。')
 if(!definition.files||typeof definition.files!=='object'||Array.isArray(definition.files)||!Object.keys(definition.files).length||Object.keys(definition.files).length>32)throw new Error('Skill 文件数量无效。')
 let total=0
 for(const [name,text] of Object.entries(definition.files)){
  if(!validPath(name)||typeof text!=='string'||bytes(text)>65536)throw new Error('无效文件或单文件超过 64 KiB：'+name)
  total+=bytes(text)
 }
 if(total>262144||!definition.files['SKILL.md']?.trim())throw new Error('需要非空 SKILL.md，所有文件合计不超过 256 KiB。')
 // Import is content only: never import execution permissions or activate unreviewed instructions.
 return {...definition,enabled:false,allow_python:false,execution_profile:null}
}
