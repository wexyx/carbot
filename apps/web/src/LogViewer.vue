<script setup>
import {ref,computed,onMounted} from 'vue'
const props=defineProps({request:Function,project:String,chat:String,file:Object})
const content=ref(''),offset=ref(0),more=ref(false),busy=ref(false),error=ref(''),raw=ref(false)
const formatted=computed(()=>content.value.split('\n').map(line=>{try{return JSON.stringify(JSON.parse(line),null,2)}catch{return line}}).join('\n'))
async function load(){
 busy.value=true;error.value=''
 try{const page=await props.request('/v1/repl/'+props.project+'/chats/'+encodeURIComponent(props.chat)+'/logs/files/'+encodeURIComponent(props.file.name)+'?offset='+offset.value);content.value=page.content;offset.value=page.next_offset;more.value=page.has_more}
 catch(e){error.value=e.message}
 finally{busy.value=false}
}
onMounted(load)
</script>
<template>
 <p>{{file.name}}</p><el-switch v-model="raw" active-text="原始 JSONL" inactive-text="格式化"/>
 <el-alert v-if="error" :title="error" type="error" :closable="false"/>
 <pre v-loading="busy" class="log-content">{{raw?content:formatted}}</pre>
 <div class="log-controls"><span>分段读取，每段最多 128 KB</span><el-button :disabled="busy" @click="offset=0;load()">从头查看</el-button><el-button v-if="more" :loading="busy" @click="load">下一段</el-button></div>
</template>
<style scoped>.log-content{white-space:pre-wrap;overflow-wrap:anywhere;overflow:auto;max-height:70vh;font:12px/1.6 monospace;background:var(--el-fill-color-light);padding:14px}.log-controls{display:flex;align-items:center;gap:10px;flex-wrap:wrap;font-size:12px;color:var(--muted)}</style>
