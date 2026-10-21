import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import test from "node:test";

// Each probe starts with an independent instance: a previous high-water mark
// must not hide an oversized input copy. No intentional host OOM is used.
for (const entry of ["node.mjs", "node.cjs"]) {
  test(`${entry}: oversized construction inputs are rejected before WASM copies`, () => {
    const script = `
    import assert from 'node:assert/strict';
    let memory;const Native=WebAssembly.Instance;
    WebAssembly.Instance=class extends Native {constructor(...args){super(...args);memory=this.exports.memory;}};
    const imported=await import(${JSON.stringify(new URL(`../dist/${entry}`, import.meta.url).href)});
    const api=imported.default??imported;
    const options={limits:{maxWorkingBytes:65536,maxPatternUnits:8}};
    const text='x'.repeat(65535);
    let getterReads=0;
    const deceptive=new Uint8Array(8*1024*1024);
    Object.defineProperty(deceptive,'length',{get(){return ++getterReads===1?0:8*1024*1024;}});
    class DeceptiveBytes extends Uint8Array {get length(){return 0;}}

    for(const operation of [
      ()=>api.AsearchIndex.fromBytes(deceptive,options),
      ()=>api.AsearchIndex.fromBytes(new DeceptiveBytes(8*1024*1024),options),
      ()=>api.AsearchIndex.fromBytes(new Uint8Array(8*1024*1024),options),
      ()=>api.AsearchIndex.fromStrings([text],options),
      ()=>api.AsearchIndex.fromDocs([{key:0,texts:[text]}],options),
      ()=>api.buildIndex([{key:0,texts:[text]}],options),
      ()=>api.buildDelta([{key:0,texts:[text]}],[0],options),
      ()=>api.buildDelta([{key:0,texts:['x'.repeat(31000)]}],Array.from({length:1000},(_,i)=>i),options),
      ()=>api.IntentIndex.fromEntries([{key:0,expr:text}],options),
    ]){
      const before=memory.buffer.byteLength;
      assert.throws(operation,{code:'RESOURCE_LIMIT'});
      assert.equal(memory.buffer.byteLength,before,'rejection must precede input allocation');
    }
    assert.equal(getterReads,0,'length getters must never run');
    const root=api.AsearchIndex.fromStrings(['ok'],{limits:{maxPatternUnits:8}});
    assert.equal(root.search('ok').length,1);root.free();
    const bounded=api.AsearchIndex.fromStrings(['ok'],{blockWords:1,limits:{maxWorkingBytes:65536,maxPatternUnits:8}});
    const ref=bounded.search('ok').refs[0];
    const before=memory.buffer.byteLength;
    assert.throws(()=>bounded.explain(ref,'ok',{original:text}),{code:'RESOURCE_LIMIT'});
    assert.equal(memory.buffer.byteLength,before);
    assert.equal(bounded.search('ok').length,1);bounded.free();
  `;
    const result = spawnSync(
      process.execPath,
      ["--input-type=module", "-e", script],
      { encoding: "utf8", timeout: 20000 },
    );
    assert.equal(result.status, 0, result.stderr);
  });
}
