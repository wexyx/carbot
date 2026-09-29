import test from 'node:test'
import assert from 'node:assert/strict'
import {conversationView} from './conversation-view.js'
test('hundreds of stream fragments produce one answer, not hundreds of cards',()=>{
  const events=[{seq:1,type:'user',content:'在哪'}]
  for(let i=0;i<311;i++)events.push({seq:i+2,type:'text_delta',text:'字'})
  events.push({seq:313,type:'completed',text:'字'.repeat(311)})
  const rows=conversationView(events)
  assert.equal(rows.filter(r=>r.type==='assistant').length,1)
  assert.equal(rows[1].text.length,311)
  assert.equal(rows.at(-1).text,'已完成')
  assert.equal(events.length,313)
})
test('tool boundaries, checkpoints, interrupted partial answers and later turns stay distinct',()=>{
  const rows=conversationView([
    {seq:1,type:'text_delta',text:'正在检查'},
    {seq:2,type:'tool_started',name:'group_list'},
    {seq:3,type:'tool_finished',output:'[]'},
    {seq:4,type:'context_checkpoint',content:'internal'},
    {seq:5,type:'text_delta',text:'部分回答'},
    {seq:6,type:'failed',message:'interrupted'},
    {seq:7,type:'user',content:'继续'},
    {seq:8,type:'completed',text:'最终回答'},
  ])
  assert.deepEqual(rows.filter(r=>r.type==='assistant').map(r=>r.text),['正在检查','部分回答','最终回答'])
  assert.ok(!rows.some(r=>r.type==='context_checkpoint'))
  assert.ok(rows.some(r=>r.type==='failed'&&r.text==='interrupted'))
})

test('members stream into separate bubbles and group aggregate never duplicates replies',()=>{
 const rows=conversationView([
 {seq:1,type:'agent.delta',agent:'alice',invocation_id:'a1',content:'A'},
 {seq:2,type:'agent.delta',agent:'bob',invocation_id:'b1',content:'B'},
 {seq:3,type:'agent.delta',agent:'alice',invocation_id:'a1',content:'!'},
 {seq:4,type:'agent.message',agent:'bob',invocation_id:'b1',content:'B!'},
 {seq:5,type:'agent.message',agent:'alice',invocation_id:'a1',content:'A!'},
 {seq:6,type:'agent.message',agent:'alice',invocation_id:'a2',content:'Next round'},
 {seq:7,type:'agent.message',aggregate:true,content:'Full transcript'},
 ])
 assert.deepEqual(rows.filter(r=>r.type==='assistant').map(r=>[r.label,r.text]),[['alice','A!'],['bob','B!'],['alice','Next round']])
})
test('live tool JSON remains a tool event, not assistant text',()=>{
 const rows=conversationView([{seq:1,type:'agent.tool.started',agent:'a',invocation_id:'a1',content:'{"name":"command_run","input":{"command":"pwd"}}'},{seq:2,type:'agent.tool.finished',agent:'a',invocation_id:'a1',content:'{"output":"done"}'}])
 assert.equal(rows.length,1);assert.equal(rows[0].type,'tool');assert.equal(rows[0].pending,false)
})
