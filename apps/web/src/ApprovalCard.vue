<script setup>
import {computed} from 'vue'
import {approvalSummary} from './approval-summary.js'
const props=defineProps({item:Object,workspace:Boolean})
const summary=computed(()=>approvalSummary(props.item,props.workspace))
const emit=defineEmits(['decide','conversation'])
function allowConversation(){if(window.confirm('本对话后续所有 Shell 命令将无需再次确认，可能修改或删除工作目录内文件并联网。目录沙箱保持，重置上下文或重启后失效。是否继续？'))emitConversation()}
const emitConversation=()=>emit('conversation')
</script>
<template>
 <section class="approval-card" role="alert">
  <div><b>{{summary.title}}</b><span>{{summary.description}}</span></div>
  <small>目标：{{workspace?(item.command?item.workdir:item.path):(item.client_id||item.input?.url||item.tool||'当前项目')}}</small>
  <details><summary>查看详情</summary><p>{{item.operation||item.tool}}</p><pre>{{item.command||item.path||item.input||item.warning}}</pre></details>
  <div class="approval-actions"><el-button type="primary" @click="$emit('decide',true)">允许一次</el-button><el-button v-if="workspace&&item.command&&item.conversation_id" type="warning" @click="allowConversation">本对话允许</el-button><el-button @click="$emit('decide',false)">拒绝</el-button></div>
 </section>
</template>
<style scoped>.approval-card{border:1px solid var(--line);border-radius:10px;padding:12px 14px;margin:8px 0;font-size:12px;background:var(--surface)}.approval-card b,.approval-card span{display:block}.approval-card span,small{color:var(--muted);margin:5px 0}summary{cursor:pointer;color:var(--muted);margin:8px 0}pre{white-space:pre-wrap;overflow-wrap:anywhere;max-height:180px;overflow:auto}.approval-actions{display:flex;gap:8px;margin-top:8px}</style>
