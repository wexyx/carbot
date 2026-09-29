import test from 'node:test'
import assert from 'node:assert/strict'
import {processView} from './process-view.js'
import {messageId} from './message-id.js'
test('HTTP origins can allocate optimistic IDs without randomUUID',()=>{
  assert.notEqual(messageId({}),messageId({}))
  assert.match(messageId(undefined),/.+/)
  assert.equal(messageId({randomUUID:()=> 'native'}),'native')
})
test('tool JSON is folded into one process row; actual JSON answers remain answers',()=>{
  const input=[{seq:1,type:'user',text:'run'},{seq:2,type:'tool',text:'{"a":1}',pending:false},{seq:3,type:'process',text:'{"trace":2}'},{seq:4,type:'tool',text:'{}',pending:false},{seq:5,type:'assistant',text:'{"answer":42}'},{seq:6,type:'status',text:'已完成'}]
  const rows=processView(input,false)
  assert.equal(rows.filter(r=>r.type==='process-group').length,1)
  assert.equal(rows[1].details.length,3)
  assert.equal(rows[2].text,'{"answer":42}')
  assert.equal(input.length,6)
})

test('compact process lines show actions without dumping JSON results', async()=>{
  const {processSummary}=await import('./process-summary.js')
  assert.equal(processSummary({type:'tool',name:'command_run',input:'{"command":"rg test"}',text:'{"secret":"result"}',pending:false},false),'已运行 rg test')
  assert.equal(processSummary({type:'tool',name:'python',pending:true},true),'正在运行 python')
  assert.equal(processSummary({type:'process',label:'context.compact',pending:true},true),'正在压缩上下文')
})
