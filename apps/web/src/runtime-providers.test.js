import test from 'node:test'
import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'

// The provider list and its settings keys are duplicated in Rust and Vue. The backend
// rejects unknown keys, so a provider offered in the UI but never persisted here is a
// silent no-op: these tests pin the two lists to each other.
const runtimeFields=readFileSync(new URL('./RuntimeConfigurationFields.vue',import.meta.url),'utf8')
const agentManagement=readFileSync(new URL('./AgentManagement.vue',import.meta.url),'utf8')
const adminConfiguration=readFileSync(new URL('./AdminConfiguration.vue',import.meta.url),'utf8')
const settings=readFileSync(new URL('../../../crates/agent-node/src/configuration/settings.rs',import.meta.url),'utf8')
const opencodeConfig=readFileSync(new URL('../../../crates/agent-runtime/src/providers/opencode/config.rs',import.meta.url),'utf8')
const agentStart=readFileSync(new URL('../../../crates/agent-node/src/management/tools/agent_start.rs',import.meta.url),'utf8')

test('every provider the runtime can build is selectable in the UI',()=>{
  const rustProviders=[...settings.matchAll(/^\s{12}"([a-z]+)" =>/gm)].map(m=>m[1])
  assert.ok(rustProviders.includes('opencode'),'backend lost the opencode provider')
  for(const provider of rustProviders){
    assert.ok(
      runtimeFields.includes(`'${provider}'`),
      `RuntimeConfigurationFields cannot select '${provider}'`,
    )
    assert.ok(
      agentManagement.includes(`&quot;${provider}&quot;`),
      `AgentManagement cannot select '${provider}'`,
    )
  }
})

test('the management tool accepts exactly the providers the UI offers',()=>{
  const offered=[...runtimeFields.matchAll(/v-for="value in \[([^\]]+)\]"/g)][0][1]
    .split(',').map(v=>v.trim().replace(/'/g,''))
  const enumValues=agentStart.match(/"enum":\[([^\]]+)\]/)[1]
    .split(',').map(v=>v.trim().replace(/"/g,''))
  assert.deepEqual(enumValues.sort(),[...offered].sort())
})

test('OpenCode exposes its launcher, model, approval and latency settings, all persisted',()=>{
  for(const key of ['OPENCODE_BIN','OPENCODE_MODEL','OPENCODE_AGENT','OPENCODE_AUTO_APPROVE','OPENCODE_THINKING','OPENCODE_STANDALONE']){
    assert.ok(settings.includes(`"${key}"`),`${key} is not a known setting`)
    // A field the form edits but never submits would look configured and change nothing.
    assert.ok(agentManagement.includes(`${key}:''`),`${key} missing from the Agent draft`)
    assert.ok(adminConfiguration.includes(`${key}:''`),`${key} missing from the admin draft`)
  }
  assert.ok(runtimeFields.includes('v-model="values.OPENCODE_BIN"'))
  assert.ok(runtimeFields.includes('v-model="values.OPENCODE_MODEL"'))
  assert.ok(runtimeFields.includes('v-model="values.OPENCODE_AGENT"'))
  assert.ok(runtimeFields.includes('values.OPENCODE_AUTO_APPROVE'))
})

test('the approval control defaults to asking, mirroring the Rust default',()=>{
  assert.match(
    opencodeConfig,/OPENCODE_AUTO_APPROVE[\s\S]{0,320}?unwrap_or\(false\)/,
    'OpenCode must not auto-approve tools unless a human opts in',
  )
  assert.ok(
    runtimeFields.includes(`values.OPENCODE_AUTO_APPROVE||'false'`),
    'the UI must show the same default the backend applies',
  )
})

// A scrubbed HOME cannot find the shared background service's registration, and the
// client then waits on starting one instead of failing fast. Measured: the shared path
// produced no output at all in 45s, while a private server answered in about 5s.
test('the private server stays the default and the UI says the same thing',()=>{
  assert.match(
    opencodeConfig,/OPENCODE_STANDALONE[\s\S]{0,320}?unwrap_or\(true\)/,
    'OpenCode must boot a private server unless a human opts into the shared service',
  )
  assert.ok(
    runtimeFields.includes(`values.OPENCODE_STANDALONE??'true'`),
    'the UI must offer the same default the backend applies',
  )
  // An unset form field must not silently change how a turn answers.
  assert.match(opencodeConfig,/OPENCODE_THINKING[\s\S]{0,320}?unwrap_or\(true\)/)
  assert.ok(runtimeFields.includes(`values.OPENCODE_THINKING??'true'`))
})
