import test from 'node:test'
import assert from 'node:assert/strict'
import {parseToolImport,parseSkillImport} from './capability-import.js'
const file=(name,text,path='')=>({name,size:new TextEncoder().encode(text).length,webkitRelativePath:path,arrayBuffer:async()=>new TextEncoder().encode(text).buffer})
test('external imports validate names, reject collisions and never auto-enable commands',()=>{
 assert.equal(parseToolImport('{"name":"check","description":"test","command":"pwd","enabled":true}')[0].enabled,false)
 assert.throws(()=>parseToolImport('[{"name":"a","description":"a","command":"pwd"},{"name":"a","description":"b","command":"pwd"}]'))
 assert.throws(()=>parseToolImport('{"name":"check","description":"test","command":"pwd","scope":"management"}'))
})
test('skill uploads preserve safe resources but do not grant execution permissions',async()=>{
 const skill=await parseSkillImport([file('SKILL.md','# Hello','review/SKILL.md'),file('test.py','print(1)','review/scripts/test.py')])
 assert.equal(skill.id,'review');assert.equal(skill.files['scripts/test.py'],'print(1)');assert.equal(skill.allow_python,false);assert.equal(skill.enabled,false)
 await assert.rejects(parseSkillImport([file('SKILL.md','# Good'),file('../secret','bad')]))
 await assert.rejects(parseSkillImport([file('readme.txt','No instructions')]))
 const imported=await parseSkillImport([file('skill.json',JSON.stringify({...skill,allow_python:true,enabled:true}))])
 assert.equal(imported.allow_python,false);assert.equal(imported.enabled,false)
})
