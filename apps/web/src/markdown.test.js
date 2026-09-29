import test from 'node:test'
import assert from 'node:assert/strict'
import {renderMarkdown} from './markdown.js'
test('Markdown renders headings, lists, fences, emphasis and tables',()=>{
 const html=renderMarkdown('# 标题\n\n**粗体**\n\n- item\n\n~~~rust\nlet n=1;\n~~~\n\n|A|B|\n|-|-|\n|1|2|')
 for(const tag of ['<h1>','<strong>','<ul>','<pre>','<table>'])assert.ok(html.includes(tag),tag)
})
test('model Markdown cannot execute HTML, javascript links or fetch images',()=>{
 const html=renderMarkdown('<script>alert(1)</script>\n[x](javascript:alert(1))\n![secret](https://example.com/tracker)')
 assert.ok(!html.includes('<script>'));assert.ok(!html.includes('href="javascript:'));assert.ok(!html.includes('<img'))
 assert.match(renderMarkdown('[docs](https://example.com)'),/noopener noreferrer/)
})
