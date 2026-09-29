import MarkdownIt from 'markdown-it'
const parser=new MarkdownIt({html:false,linkify:false,breaks:true,typographer:false})
// Model text is untrusted. Never execute HTML or fetch embedded remote images.
parser.renderer.rules.image=(tokens,index)=>parser.utils.escapeHtml('[图片：'+(tokens[index].content||'image')+']')
const linkOpen=parser.renderer.rules.link_open||((tokens,index,options,env,self)=>self.renderToken(tokens,index,options))
parser.renderer.rules.link_open=(tokens,index,options,env,self)=>{
  tokens[index].attrSet('target','_blank');tokens[index].attrSet('rel','noopener noreferrer')
  return linkOpen(tokens,index,options,env,self)
}
export const renderMarkdown=text=>parser.render(String(text??''))
