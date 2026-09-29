<script setup>
import {ref,onBeforeUnmount} from 'vue'
import {Paperclip} from '@element-plus/icons-vue'
import {validateAttachment} from './attachments.js'
const props=defineProps({modelValue:{type:Array,default:()=>[]},request:Function,disabled:Boolean})
const emit=defineEmits(['update:modelValue','uploading','error'])
const input=ref(null),busy=ref(false)
let alive=true
onBeforeUnmount(()=>{alive=false})
async function add(files){
 if(props.disabled||busy.value)return
 busy.value=true;emit('uploading',true)
 try{
  let items=[...props.modelValue]
  for(const file of files){
   validateAttachment(file,items.length)
   const result=await props.request('/v1/attachments?name='+encodeURIComponent(file.name),{method:'POST',headers:{'content-type':'application/octet-stream'},body:file})
   if(!alive)return
   items=[...items,result];emit('update:modelValue',items)
  }
 }catch(e){if(alive)emit('error',e.message)}finally{if(alive){busy.value=false;emit('uploading',false)}if(input.value)input.value.value=''}
}
function drop(event){if(event.dataTransfer?.files?.length){event.preventDefault();add(Array.from(event.dataTransfer.files))}}
function paste(event){const files=Array.from(event.clipboardData?.files||[]);if(files.length){event.preventDefault();add(files)}}
defineExpose({drop,paste})
</script>
<template>
 <div class="attachment-picker">
  <input ref="input" type="file" multiple hidden @change="add(Array.from($event.target.files||[]))">
  <el-button text :icon="Paperclip" :loading="busy" :disabled="disabled||busy" @click="input.click()">{{busy?'上传中':'添加附件'}}</el-button>
  <el-tag v-for="(item,index) in modelValue" :key="item.attachment.id" closable :disable-transitions="true" @close="!busy&&!disabled&&emit('update:modelValue',modelValue.filter((_,i)=>i!==index))">{{item.attachment.name}}</el-tag>
  
 </div>
</template>
<style scoped>
.attachment-picker{display:flex;align-items:center;flex-wrap:wrap;gap:8px;margin:0 0 0 auto;min-width:0;justify-content:flex-end;max-width:65%}.attachment-picker>span:not(.el-tag){color:var(--muted);font-size:12px}.attachment-picker .el-tag{max-width:240px}.attachment-picker :deep(.el-tag__content){overflow:hidden;text-overflow:ellipsis}
</style>
