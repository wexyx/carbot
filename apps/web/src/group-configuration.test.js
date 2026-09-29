import test from 'node:test'
import assert from 'node:assert/strict'
import {validateGroupConfiguration} from './group-configuration.js'
test('complete group configuration validates modes, leader, order and limits',()=>{
 const value={name:'Team',policy:{mode:'pmo',members:[{path:['a'],role:'owner'}],leader:null,rounds:2,instructions:'plan'}}
 assert.match(validateGroupConfiguration(value),/Leader/)
 value.policy.leader=['a'];assert.equal(validateGroupConfiguration(value),'')
 value.policy.relay_strategy='random';assert.equal(validateGroupConfiguration(value),'')
 value.policy.members.push({path:['a','child'],role:'worker'});assert.match(validateGroupConfiguration(value),/重叠/)
})

test('simple chat requires exactly one Agent and permits automatic titles',()=>{
 const draft={name:'',policy:{mode:'chat',members:[{path:['worker'],role:'assistant'}],leader:null,rounds:1,instructions:''}}
 assert.equal(validateGroupConfiguration(draft),'')
 draft.policy.members.push({path:['second'],role:'reviewer'})
 assert.match(validateGroupConfiguration(draft),/一个 Agent/)
 draft.policy.members=[]
 assert.match(validateGroupConfiguration(draft),/一个 Agent/)
})
