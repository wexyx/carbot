export function peerManagementUrl(value) {
 try {
  const url=new URL(value)
  if(!['http:','https:'].includes(url.protocol)||url.username||url.password||url.search||url.hash)return ''
  return url.href
 } catch {return ''}
}
