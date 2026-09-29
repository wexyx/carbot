<script setup>
import {Tools,Loading,Check} from '@element-plus/icons-vue'
import {ref,computed,watch,nextTick} from 'vue'
import {messageClock,messageDate} from './chat-time.js'
import {processSummary} from './process-summary.js'
import {processView} from './process-view.js'
import MarkdownText from './MarkdownText.vue'
const props=defineProps({messages:Array,running:Boolean,management:Boolean,emptyTitle:String,emptyDescription:String,hasMore:Boolean,loadOlder:Function,progress:String})
const emit=defineEmits(['suggest','history-error'])
const loadingOlder=ref(false)
async function loadEarlier(){
 const el=scroller.value
 if(!el||loadingOlder.value||!props.hasMore||!props.loadOlder)return
 loadingOlder.value=true;follow.value=false
 const height=el.scrollHeight,top=el.scrollTop
 try{await props.loadOlder();await nextTick();el.scrollTop=top+el.scrollHeight-height}catch(e){emit('history-error',e.message)}finally{loadingOlder.value=false}
}
const displayRows=computed(()=>{let previous='';return processView(props.messages,props.running).map(row=>{const day=messageDate(row.timestamp);const divider=day&&day!==previous;if(day)previous=day;return {...row,day:divider?day:''}})})
const historyRows=computed(()=>displayRows.value.filter(row=>!(row.type==='process-group'&&row.pending)))
const liveRows=computed(()=>displayRows.value.filter(row=>row.type==='process-group'&&row.pending).flatMap(row=>row.details))
const activeProcess=computed(()=>displayRows.value.some(row=>row.type==='process-group'&&row.pending))
const scroller=ref(null),follow=ref(true)
function scroll(){const el=scroller.value;if(!el||loadingOlder.value)return;follow.value=el.scrollHeight-el.scrollTop-el.clientHeight<100;if(el.scrollTop<60)loadEarlier()}
watch(()=>[props.messages,props.running],async()=>{await nextTick();if(!loadingOlder.value&&follow.value&&scroller.value)scroller.value.scrollTop=scroller.value.scrollHeight},{deep:true})
</script>
<template>
  <div ref="scroller" class="messages" @scroll="scroll" @wheel.passive="event=>{if(event.deltaY<0&&scroller.scrollTop<60)loadEarlier()}">
    <div class="message-column">
      <p v-if="hasMore" class="history-status">{{loadingOlder?'正在加载更早的消息…':'向上滚动加载更早的消息'}}</p>
      <div v-if="!messages.length" class="welcome">
        <h1>{{emptyTitle||(management?'管理':'暂无消息')}}</h1>
        <p>{{emptyDescription||(management?'配置 Agent、创建项目，或查看节点与项目状态。':'在下方发送消息开始协作，交流方式可在项目配置中调整。')}}</p>
        <div v-if="management" class="suggestions"><el-button type="primary" native-type="button" @click="$emit('suggest','看看当前有哪些 Agent 和群组')">查看我的 Agent <span>↗</span></el-button><el-button type="primary" native-type="button" @click="$emit('suggest','帮我设计一个开发协作群，先给出方案')">组建一个协作群 <span>↗</span></el-button></div>
      </div>
      <template v-for="r in historyRows" :key="r.seq">
        <div v-if="r.day" class="chat-date-divider"><span>{{r.day}}</span></div>
        <div v-if="r.type==='process-group'" class="tool-wrap">
          <details v-for="detail in r.details" :key="detail.seq" class="process-line">
            <summary><el-icon :class="{'is-loading':r.pending&&detail.pending}"><Loading v-if="r.pending&&detail.pending"/><Tools v-else/></el-icon><span>{{processSummary(detail,r.pending)}}</span></summary>
            <div class="tool-content"><b>{{detail.label||detail.name||'执行记录'}}</b><template v-if="detail.input"><small>输入</small><pre>{{detail.input}}</pre></template><pre>{{detail.text||'等待返回…'}}</pre></div>
          </details>
        </div>
        <div v-else-if="r.type==='tool'" class="tool-wrap">
          <el-collapse class="tool-collapse"><el-collapse-item :name="r.seq"><template #title><el-icon><Tools/></el-icon><span class="tool-name">{{r.label}}</span><time v-if="r.timestamp" :datetime="r.timestamp" :title="new Date(r.timestamp).toLocaleString()">{{messageClock(r.timestamp)}}</time><el-tag size="small" :type="r.pending?'warning':'info'">{{r.pending?'执行中':'已完成'}}</el-tag></template>
            <div class="tool-content"><template v-if="r.input"><b>输入</b><pre>{{r.input}}</pre></template><b>结果</b><pre>{{r.text||'等待返回…'}}</pre></div>
          </el-collapse-item></el-collapse>
        </div>
        <p v-else-if="r.type==='status'" class="turn-status">{{r.text}}</p>
        <div v-else :class="['message-row',['user','message.created'].includes(r.type)?'from-user':'from-agent']">
          <el-avatar class="avatar" :size="32" shape="square">{{['user','message.created'].includes(r.type)?'我':(r.label||'AI').slice(0,2)}}</el-avatar>
          <div class="message-content"><span class="sender">{{r.label}}<time v-if="r.timestamp" :datetime="r.timestamp" :title="new Date(r.timestamp).toLocaleString()">{{messageClock(r.timestamp)}}</time></span><div :class="['bubble',{'failure':['failed','agent.error'].includes(r.type)}]"><MarkdownText :text="r.text" /></div></div>
        </div>
      </template>

    </div>
  </div>
  <div v-if="running" class="live-process" aria-live="polite">
    <details v-for="detail in liveRows" :key="detail.seq" class="process-line">
      <summary><el-icon :class="{'is-loading':detail.pending}"><Loading v-if="detail.pending"/><Tools v-else/></el-icon><span>{{processSummary(detail,true)}}</span></summary>
      <div class="tool-content"><pre v-if="detail.input">{{detail.input}}</pre><pre>{{detail.text||'等待返回…'}}</pre></div>
    </details>
    <p v-if="!liveRows.some(d=>d.pending)" class="thinking" role="status"><el-icon class="is-loading"><Loading/></el-icon> {{progress||'思考中…'}}</p>
  </div>
</template>

<style scoped>.live-process{width:calc(100% - 56px);max-width:940px;align-self:center;max-height:160px;overflow:auto;flex-shrink:0;padding:0 16px}.live-process .thinking{margin:5px 0;font-size:13px}@media(max-width:750px){.live-process{width:calc(100% - 24px)}}</style>
<style scoped>.messages{overflow-anchor:none}.history-status{text-align:center;color:var(--muted);font-size:12px;margin:0 0 14px} .process-line{color:var(--muted);font-size:13px;margin:5px 0}.process-line summary{display:flex;align-items:center;gap:8px;cursor:pointer;list-style:none;padding:4px 0;min-width:0}.process-line summary::-webkit-details-marker{display:none}.process-line summary span{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;min-width:0}.process-line[open] summary{color:var(--text)}.process-line .el-icon{flex-shrink:0}.process-line .tool-content{margin:6px 0 12px 22px}</style>

<style scoped>.execution-status{flex-shrink:0;padding:8px 24px;color:var(--muted);font-size:12px;border-top:1px solid var(--line)}</style>
