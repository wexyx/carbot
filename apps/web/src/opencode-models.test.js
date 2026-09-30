import test from 'node:test'
import assert from 'node:assert/strict'
import {modelPrice,modelLabel,orderModels,readCatalog} from './opencode-models.js'

const free={id:'opencode/space-bunny-free',free:true,priced:true,input:0,output:0,context:1048576,tools:true,status:'active'}
const paid={id:'opencode/gpt-6.1-sol',free:false,priced:true,input:2,output:10,context:1050000,tools:true,status:'active'}
const bare={id:'opencode/big-pickle',free:false,priced:false,context:null,tools:true}

test('free is only claimed when the catalog actually priced the model at zero',()=>{
  assert.deepEqual(modelPrice(free),{free:true,text:'免费',needsAccount:false})
  assert.deepEqual(modelPrice(paid),{free:false,text:'$2 / $10 每百万 token',needsAccount:true})
  // No price at all must never read as free.
  assert.deepEqual(modelPrice(bare),{free:false,text:'价格未知',needsAccount:true})
  assert.deepEqual(modelPrice({priced:true,free:false,input:2}),{free:false,text:'价格未知',needsAccount:true})
  assert.equal(modelPrice(null).free,false)
})

test('labels carry price, context and any capability or status caveat',()=>{
  assert.equal(modelLabel(free),'opencode/space-bunny-free · 免费 · 1049K 上下文')
  assert.equal(modelLabel({...paid,tools:false,status:'beta'}),
    'opencode/gpt-6.1-sol · $2 / $10 每百万 token · 1050K 上下文 · 不支持工具调用 · beta')
  assert.equal(modelLabel(bare),'opencode/big-pickle · 价格未知')
  assert.equal(modelPrice({priced:true,free:false,input:0.125,output:1.5}).text,'$0.13 / $1.5 每百万 token')
})

test('free models lead the picker, then the cheapest',()=>{
  const ordered=orderModels([paid,bare,free,{...paid,id:'opencode/cheap',input:0.1,output:0.5}])
  assert.deepEqual(ordered.map(m=>m.id),['opencode/space-bunny-free','opencode/cheap','opencode/gpt-6.1-sol','opencode/big-pickle'])
  assert.deepEqual(orderModels(null),[])
})

test('a signed-out or empty catalog explains itself instead of offering nothing silently',()=>{
  assert.deepEqual(readCatalog({signed_in:false,priced:true,models:[]}),
    {models:[],priced:false,signedIn:false,reason:'未检测到已登录的 OpenCode，请先运行 opencode auth login，或手动填写模型 ID'})
  assert.equal(readCatalog({signed_in:true,priced:true,models:[]}).reason,'OpenCode 未返回任何模型')
  const identifiers=readCatalog({signed_in:true,priced:false,models:[bare]})
  assert.equal(identifiers.priced,false)
  assert.equal(identifiers.reason,'')
  assert.equal(identifiers.models[0].id,'opencode/big-pickle')
})
