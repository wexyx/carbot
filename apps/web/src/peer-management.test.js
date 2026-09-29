import test from 'node:test'
import assert from 'node:assert/strict'
import {peerManagementUrl} from './peer-management.js'
test('management navigation accepts only explicit safe web URLs',()=>{
 assert.equal(peerManagementUrl('http://host:8080'),'http://host:8080/')
 for(const value of ['javascript:alert(1)','file:///tmp','http://u:p@host','127.0.0.1:5555',null])assert.equal(peerManagementUrl(value),'')
})
