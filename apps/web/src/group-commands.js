export const groupCommands = [
  {name:'/new',usage:'/new',description:'开启新上下文，保留历史'}, 
  {name:'/members',usage:'/members',description:'查看群成员（同 CLI）'},
  {name:'/agents',usage:'/agents',description:'查看群成员'},
  {name:'/add-agent',usage:'/add-agent PATH [角色]',description:'添加已连接的 Agent'},
  {name:'/remove-agent',usage:'/remove-agent PATH',description:'移除群成员'},
  {name:'/agent',usage:'/agent PATH role 角色',description:'配置角色；也支持 leader、provider、start'},
  {name:'/group',usage:'/group mode chat|relay|a2a|pmo',description:'配置交流模式；instructions 修改协作要求'},
  {name:'/help',usage:'/help',description:'列出群聊命令'},
]
export function commandSuggestions(text) {
  const value=text.trimStart()
  if(!value.startsWith('/'))return []
  const name=value.split(/\s/)[0]
  return groupCommands.filter(c=>c.name.startsWith(name))
}
export function validateGroupCommand(text) {
  const [name,...args]=text.trim().split(/\s+/)
  if(!groupCommands.some(c=>c.name===name))return '未知群聊命令，请输入 /help 查看支持的命令。'
  if(['/add-agent','/remove-agent','/agent'].includes(name)&&!args.length)return '请选择 Agent 路径。可先用 /agents 查看成员，添加时填写已连接 Agent 的路径。'
  if(name==='/group'&&args.length<2)return '用法：/group mode chat|relay|a2a|pmo 或 /group instructions 协作要求'
  return ''
}
