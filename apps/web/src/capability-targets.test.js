import test from 'node:test'
import assert from 'node:assert/strict'
import {localCapabilityAgents} from './capability-targets.js'
test('capability targets are local only and project selection limits membership',()=>{
 const agents=[{id:'a',kind:'local'},{id:'b',kind:'local'},{id:'r',kind:'remote'},{id:'v',kind:'virtual'}]
 assert.deepEqual(localCapabilityAgents(agents).map(a=>a.id),['a','b'])
 assert.deepEqual(localCapabilityAgents(agents,{body:{policy:{members:[{path:['b']},{path:['r','a']},{path:['v']}]}}}).map(a=>a.id),['b'])
 assert.deepEqual(localCapabilityAgents(agents,{body:{policy:{members:[]}}}),[])
})
