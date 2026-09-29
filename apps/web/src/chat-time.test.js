import test from 'node:test'
import assert from 'node:assert/strict'
import {eventTime,messageClock,messageDate} from './chat-time.js'
import {conversationView} from './conversation-view.js'
test('timestamps support seconds, milliseconds, ISO and missing legacy dates',()=>{
 assert.equal(eventTime({logged_at:1700000000}),'2023-11-14T22:13:20.000Z')
 assert.equal(eventTime({created_at:'1700000000'}),eventTime({timestamp:1700000000000}))
 assert.equal(eventTime({timestamp:'invalid'}),null)
 assert.equal(eventTime({}),null)
 assert.equal(messageClock(null),'');assert.equal(messageDate(null),'')
 const rows=conversationView([{seq:1,type:'text_delta',text:'A',logged_at:1700000000},{seq:2,type:'text_delta',text:'B',logged_at:1700000010},{seq:3,type:'completed',text:'AB',logged_at:1700000020}])
 assert.equal(rows[0].timestamp,'2023-11-14T22:13:20.000Z');assert.equal(rows[0].text,'AB')
})
