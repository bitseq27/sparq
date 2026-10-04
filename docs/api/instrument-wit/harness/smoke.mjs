// smoke.mjs — the first host: drive the noop instrument component through the full lifecycle.
import { guest } from './gen/noop-component.js';

let failures = 0;
const unwrap = (e) => e?.payload ?? e;
const check = (name, cond, detail = '') => {
  console.log(`${cond ? 'PASS' : 'FAIL'}  ${name}${detail !== '' ? ' — ' + detail : ''}`);
  if (!cond) failures++;
};
const res = (frames) => ({
  sampleRate: 48000, blockFrames: frames,
  audioInChannels: Uint32Array.from([2]), audioOutChannels: Uint32Array.from([2]),
  oversampling: 'none', voices: 0, arenaBytes: 0n, fuelPerBlock: 1000000n,
  tokenBundle: { major: 1, minor: 1, patch: 0 },
});

check('id()', guest.id() === 'example/syn/noop', guest.id());
guest.prepare(res(64));
check('prepare(64 frames) accepted', true);

let e = null;
try { guest.prepare(res(0)); } catch (err) { e = err; }
check('prepare(0 frames) refuses in words', !!e && (unwrap(e)?.tag === 'resources' && unwrap(e)?.val?.includes?.('block-frames')), JSON.stringify(e));
guest.prepare(res(64)); // re-prime

const out = guest.process({
  blockId: 0n, frames: 64, tSample: 0n, tick: 0n, ppqn: 960,
  params: { version: 1n, values: new Float32Array([]) },
  audioIn: [{ channels: 2, connected: true, interleaved: new Float32Array(128) }],
  cvIn: [{ tag: 'per-block', val: 0.5 }],
  eventsIn: [], dataIn: [],
});
check('process -> silenced', out.status === 'silenced', String(out.status));
check('process audio shape 1×(2ch×64f)', out.audioOut.length === 1 && out.audioOut[0].interleaved.length === 128);
check('process audio exact zeros', Array.from(out.audioOut[0].interleaved).every((v) => v === 0));
check('process cv-out per-block 0.0', out.cvOut.length === 1 && out.cvOut[0].tag === 'per-block' && out.cvOut[0].val === 0);

const surf = guest.draw({ displayId: 'view', widthPx: 160, heightPx: 96, lod: 'full', timeSec: 1.5 });
check('draw -> at-rest empty items', surf.tag === 'items' && surf.val.length === 0);

e = null; try { guest.configure(new Uint8Array([1, 2, 3])); } catch (err) { e = err; }
check('configure(non-empty) refuses in words', !!e && (unwrap(e)?.tag === 'state'), JSON.stringify(e));
guest.configure(new Uint8Array());
check('configure(empty) ok + saveState empty', guest.saveState().length === 0);
e = null; try { guest.message(new Uint8Array([9])); } catch (err) { e = err; }
check('message refuses in words', !!e && (unwrap(e)?.tag === 'message'), JSON.stringify(e));

console.log(failures === 0 ? 'SMOKE PASS (all checks)' : `SMOKE FAIL (${failures})`);
process.exit(failures === 0 ? 0 : 1);
