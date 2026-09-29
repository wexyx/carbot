<script setup>
import AgentEnvironmentFields from './AgentEnvironmentFields.vue'
const provider=defineModel('provider',{type:String})
const secret=defineModel('secret',{type:String,default:''})
const clearSecret=defineModel('clearSecret',{type:Boolean,default:false})
defineProps({values:{type:Object,required:true},hasKey:Boolean,showProvider:{type:Boolean,default:true}})
</script>
<template>
 <div class="runtime-fields">
  <el-form-item v-if="showProvider" label="运行器"><el-select v-model="provider" aria-label="配置运行器"><el-option v-for="value in ['carbot','codex','claude','mock']" :key="value" :value="value" :label="value"/></el-select></el-form-item>
  <template v-if="provider==='carbot'">
   <el-form-item label="模型厂商"><el-select v-model="values.MODEL_PROVIDER" aria-label="模型厂商"><el-option v-for="value in ['openai','anthropic','gemini','deepseek','qwen','ark','ollama','compatible']" :key="value" :value="value" :label="value"/></el-select></el-form-item>
   <el-form-item label="上下文长度"><el-input v-model="values.CONTEXT_MAX_TOKENS" placeholder="65536（含输出预留）" aria-label="上下文长度"/></el-form-item>
   <el-form-item label="模型名称"><el-input v-model="values.MODEL_NAME" required placeholder="例如 deepseek-chat" aria-label="模型名称"/></el-form-item>
   <el-form-item label="接口地址"><el-input v-model="values.MODEL_BASE_URL" placeholder="留空使用厂商默认地址" aria-label="接口地址"/></el-form-item>
   <el-form-item label="接口协议"><el-select v-model="values.MODEL_API" aria-label="接口协议"><el-option value="" label="厂商默认"/><el-option v-for="value in ['chat','responses','anthropic']" :key="value" :value="value" :label="value"/></el-select></el-form-item>
   <el-form-item label="API Key"><el-input v-model="secret" type="password" autocomplete="new-password" :placeholder="hasKey?'已设置，留空保留':'填写密钥（不回显已保存内容）'" aria-label="模型 API Key"/></el-form-item>
   <el-form-item label="密钥操作"><el-checkbox v-model="clearSecret">清除已有 API Key</el-checkbox></el-form-item>
  </template>
  <el-form-item v-else-if="provider==='codex'" label="Codex 启动命令"><el-input v-model="values.CODEX_BIN" placeholder="codex 或 codex --model 模型名" aria-label="Codex 启动命令"/><small>支持命令名、完整路径及参数；含空格的路径请加引号。不需要填写 exec、输出格式或权限参数。</small></el-form-item>
  <el-form-item v-else-if="provider==='claude'" label="Claude 启动命令"><el-input v-model="values.CLAUDE_BIN" placeholder="claude 或 claude --model sonnet" aria-label="Claude 启动命令"/><small>支持命令名、完整路径及参数；含空格的路径请加引号。不需要填写 -p、输出格式或权限参数。</small></el-form-item>
  <p v-else class="runtime-note">Mock 仅验证通信链路，不调用模型。</p>
  <AgentEnvironmentFields v-if="provider!=='mock'" :values="values"/>
 </div>
</template>
<style scoped>
.runtime-fields{min-width:0;width:100%;box-sizing:border-box;display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:18px 24px;grid-column:1/-1}
.el-form-item{margin:0;min-width:0}.runtime-note{font-size:12px;color:var(--muted);align-self:center}
@media(max-width:650px){.runtime-fields{grid-template-columns:1fr}}
</style>
