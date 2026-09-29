import {readFile,appendFile} from 'node:fs/promises'
import {join} from 'node:path'

// Inspect durable state without requiring a periodically rewritten snapshot.
export async function readState(dir){
  let state={format_version:1,sequence:0,collections:{}}
  try{state=JSON.parse(await readFile(join(dir,'state.json'),'utf8'))}catch(e){if(e.code!=='ENOENT')throw e}
  let journal=''
  try{journal=await readFile(join(dir,'state.jsonl'),'utf8')}catch(e){if(e.code!=='ENOENT')throw e}
  const committed=journal.slice(0,journal.lastIndexOf('\n')+1)
  for(const line of committed.split('\n').filter(Boolean)){
    const row=JSON.parse(line)
    if(row.version!==1||row.sequence!==state.sequence+1)throw Error('Invalid state journal')
    for(const change of row.changes){
      const collection=state.collections[change.collection]??={}
      if(change.deleted)delete collection[change.key];else collection[change.key]=change.value
    }
    state.sequence=row.sequence
  }
  return state
}

// Trusted fixture-only changes while the instance is stopped.
export async function changeState(dir,collection,key,value){
  const state=await readState(dir)
  await appendFile(join(dir,'state.jsonl'),JSON.stringify({version:1,sequence:state.sequence+1,changes:[{collection,key,value,deleted:false}]})+'\n')
}
