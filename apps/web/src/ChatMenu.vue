<script setup>
import {ref} from 'vue'
import {MoreFilled} from '@element-plus/icons-vue'
const props=defineProps({group:Object,request:Function})
const emit=defineEmits(['changed','deleted'])
const mode=ref(''),name=ref(''),busy=ref(false),error=ref('')
function open(value){mode.value=value;name.value=props.group.body.name;error.value=''}
async function save(){
 busy.value=true;error.value=''
 try{
  const deleting=mode.value==='delete'
  await props.request(`/v1/repl/${props.group.namespace_id}/groups/${props.group.key}${deleting?'':'/name'}`,{method:deleting?'DELETE':'PUT',headers:{'content-type':'application/json'},body:JSON.stringify({expected_version:props.group.version,name:name.value})})
  mode.value='';emit(deleting?'deleted':'changed',props.group.key)
 }catch(e){error.value=e.message}finally{busy.value=false}
}
</script>
<template>
 <el-dropdown trigger="click" @command="open"><el-button text class="chat-menu" :aria-label="group.body.name+'更多操作'"><el-icon><MoreFilled/></el-icon></el-button><template #dropdown><el-dropdown-menu><el-dropdown-item command="rename">重命名</el-dropdown-item><el-dropdown-item command="delete" divided>删除聊天</el-dropdown-item></el-dropdown-menu></template></el-dropdown>
 <el-dialog :model-value="!!mode" :title="mode==='delete'?'删除聊天？':'重命名聊天'" width="min(440px,95vw)" append-to-body :close-on-click-modal="!busy" :before-close="done=>{if(!busy){mode='';done()}}">
  <el-alert v-if="error" type="error" :closable="false" :title="error"/>
  <p v-if="mode==='delete'">将从聊天列表移除“{{group.body.name}}”。本地历史日志保留，不删除 Agent 或其它聊天。</p>
  <el-input v-else v-model="name" aria-label="聊天名称" maxlength="256" :disabled="busy" @keydown.enter.prevent="save"/>
  <template #footer><el-button :disabled="busy" @click="mode=''">取消</el-button><el-button :type="mode==='delete'?'danger':'primary'" :loading="busy" :disabled="mode==='rename'&&!name.trim()" @click="save">{{mode==='delete'?'删除':'保存'}}</el-button></template>
 </el-dialog>
</template>
