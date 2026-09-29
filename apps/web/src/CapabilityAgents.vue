<script setup>
import {ref,watch,onBeforeUnmount} from 'vue'
import {localCapabilityAgents} from './capability-targets.js'
const props=defineProps({request:Function,project:String,scope:String,kind:String,resource:String,projects:Array})
const emit=defineEmits(['changed'])
const group=ref(''),rows=ref([]),loading=ref(false),saving=ref(''),error=ref('')
let generation=0
const context=()=>props.projects?.find(p=>p.key===group.value)?.namespace_id||props.project
const endpoint=()=>`/v1/repl/${context()}/capabilities/${props.scope}/${props.kind}`
const query=id=>'?agent='+encodeURIComponent(id)+(group.value?'&group='+encodeURIComponent(group.value):'')
async function load(){
 const version=++generation;loading.value=true;rows.value=[];error.value=''
 try{
  const index=await props.request(`/v1/repl/${context()}/agents/candidates`)
  const agents=props.scope==='management'?[{id:'admin',name:'管理 Agent',kind:'local',online:true}]:localCapabilityAgents(index.agents,props.projects?.find(p=>p.key===group.value))
  const result=await Promise.all(agents.map(async agent=>{
   if(agent.kind!=='local')return {...agent,readonly:true}
   const result=await props.request(endpoint()+query(agent.id))
   return {...agent,resolution:result.rows.find(r=>r.resource.id===props.resource)?.resolution}
  }))
  if(version===generation)rows.value=result
 }catch(e){if(version===generation)error.value=e.message}finally{if(version===generation)loading.value=false}
}
async function toggle(row,enabled){
 if(saving.value||loading.value)return
 saving.value=row.id;error.value=''
 try{
  const layer=row.resolution.layers.find(l=>l.layer===(group.value?'project_agent':'agent'))
  await props.request(endpoint()+'/bindings'+query(row.id),{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify({id:props.resource,layer:layer.layer,enabled,expected_version:layer.version})})
  await load();emit('changed')
 }catch(e){error.value=e.message}finally{saving.value=''}
}
watch(()=>[props.resource,props.kind,props.scope,group.value],load,{immediate:true})
onBeforeUnmount(()=>generation++)
</script>
<template>
 <section class="agent-bindings">
  <el-alert v-if="error" type="error" :closable="false" :title="error"/>
  <label v-if="scope==='business'" class="binding-context">适用范围
   <el-select v-model="group" :disabled="loading||!!saving"><el-option value="" label="所有项目"/><el-option v-for="p in projects" :key="p.key" :value="p.key" :label="p.body.name||'未命名项目'"/></el-select>
  </label>
  <p>直接切换每个 Agent 的启用状态，自动保存，下一轮执行生效。{{group?'仅修改所选项目。':'修改 Agent 的跨项目默认状态；已有项目专属设置保留。'}}</p>
  <el-table :data="rows" row-key="id" :empty-text="loading?'正在加载…':'暂无 Agent'">
   <el-table-column label="Agent" min-width="170"><template #default="{row}"><strong>{{row.name||row.id}}</strong><small class="agent-role">{{row.role}}</small></template></el-table-column>
   <el-table-column label="运行状态" width="105"><template #default="{row}"><el-tag size="small" :type="row.online?'success':'info'">{{row.online?'在线':'已停止'}}</el-tag></template></el-table-column>
   <el-table-column label="启用能力" width="150"><template #default="{row}"><el-switch v-if="!row.readonly" :model-value="!!row.resolution?.enabled" :loading="saving===row.id" :disabled="loading||!!saving||!row.resolution" :aria-label="(row.name||row.id)+'启用能力'" @change="toggle(row,$event)"/><span v-else class="readonly">{{row.kind==='remote'?'远端只读':'由成员 Agent 配置'}}</span></template></el-table-column>
  </el-table>
 </section>
</template>
<style scoped>
.binding-context{display:flex;align-items:center;gap:16px;margin:20px 0}.binding-context .el-select{flex:1}.agent-bindings p,.readonly,.agent-role{color:var(--muted);font-size:12px;line-height:1.7}.agent-role{display:block;margin-top:4px}
</style>
