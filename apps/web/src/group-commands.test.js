import test from 'node:test'
import assert from 'node:assert/strict'
import {commandSuggestions,validateGroupCommand} from './group-commands.js'
test('group commands suggest syntax and reject incomplete commands before submission',()=>{
 assert.equal(commandSuggestions('/').length,8)
 assert.equal(commandSuggestions('/mem')[0].name,'/members')
 assert.equal(validateGroupCommand('/members'),'')
 assert.equal(validateGroupCommand('/new'),'')
 assert.equal(commandSuggestions('/add')[0].name,'/add-agent')
 assert.equal(commandSuggestions('normal').length,0)
 assert.ok(validateGroupCommand('/add-agent'))
 assert.ok(validateGroupCommand('/unknown'))
 assert.equal(validateGroupCommand('/add-agent worker reviewer'),'')
})
