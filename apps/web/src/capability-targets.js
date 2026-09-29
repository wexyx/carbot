export function localCapabilityAgents(agents, group) {
 const members=group ? new Set((group.body?.policy?.members||[]).filter(m=>m.path?.length===1).map(m=>m.path[0])) : null
 return agents.filter(a=>(a.kind==='local'||(!a.kind&&a.client_id))&&(!members||members.has(a.id||a.client_id)))
}
