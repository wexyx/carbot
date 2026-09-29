<script setup>
import {ref,watch} from 'vue'
const props=defineProps({request:Function,project:String,agent:String,compact:Boolean})
const emit=defineEmits(['changed'])
const current=ref(null),busy=ref(false),error=ref(''),confirm=ref(false),editing=ref(false)
let epoch=0
const path=()=>`/v1/repl/${props.project}/agents/${encodeURIComponent(props.agent)}/permissions`
watch(()=>[props.project,props.agent],async()=>{const version=++epoch;current.value=null;confirm.value=false;editing.value=false;busy.value=false;error.value='';try{const row=await props.request(path());if(version===epoch)current.value=row}catch(e){if(version===epoch)error.value=e.message}},{immediate:true})
async function save(mode){
 if(!current.value||busy.value)return
 const version=epoch,url=path()
 busy.value=true;error.value=''
 try{const result=await props.request(url,{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify({mode,expected_version:current.value.version,confirm_full_access:mode==='full'})});if(version!==epoch)return;current.value=result;confirm.value=false;emit('changed')}
 catch(e){if(version===epoch){error.value=e.message;try{const latest=await props.request(url);if(version===epoch)current.value=latest}catch{}}}
 finally{if(version===epoch)busy.value=false}
}
function choose(mode){if(mode==='full'){confirm.value=true}else save(mode)}
</script>
<template>
 <div class="permission-control" :class="{compact}" title="仅此 Agent · 下个项目任务生效">
  <span>执行权限</span><el-select v-if="current" :model-value="current.mode" @change="choose" :disabled="busy" aria-label="执行权限模式"><el-option label="请求批准" value="ask"/><el-option label="帮我批准" value="auto"/><el-option label="完全访问" value="full"/></el-select>
  <small v-if="!compact">仅此 Agent · 下个项目任务生效</small><p v-if="error" role="alert">{{error}}</p>
 </div>
 <el-dialog v-model="confirm" title="开启完全访问？" width="min(520px,94vw)" append-to-body :close-on-click-modal="false">
  <p>Agent 将跳过执行确认和目录沙箱，可读写系统当前用户有权访问的文件并联网，包括工作目录外的文件和凭据。仅对可信任务使用。</p>
  <p>已经运行的任务保持原权限；管理聊天和管理工具的审批不受此设置影响。</p>
  <template #footer><el-button :disabled="busy" @click="confirm=false">取消</el-button><el-button type="danger" :loading="busy" @click="save('full')">我了解风险，开启完全访问</el-button></template>
 </el-dialog>
</template>
<style scoped>.permission-control{display:flex;gap:10px;align-items:center;flex-wrap:wrap;font-size:12px;margin:10px 0;color:var(--muted)}.el-select{width:145px}.permission-control p{color:var(--danger);width:100%}small{font-size:11px}.permission-control.compact{margin:0;gap:6px;flex-wrap:nowrap}.compact .el-select{width:116px}.compact>span{white-space:nowrap}</style>
