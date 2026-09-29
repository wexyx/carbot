import test from 'node:test'
import assert from 'node:assert/strict'
import {approvalSummary} from './approval-summary.js'
test('approval summary gives a category without echoing source or claiming safety',()=>{
 const summary=approvalSummary({command:'python3 -c "SECRET_SOURCE"'},true)
 assert.equal(summary.title,'运行 Python 脚本')
 assert.ok(!JSON.stringify(summary).includes('SECRET_SOURCE'))
 assert.match(summary.description,/不是安全结论/)
 assert.match(approvalSummary({command:'unknown --arg'},true).title,/Shell/)
})
