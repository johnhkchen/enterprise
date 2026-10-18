// Runs a wasm32-wasip1 program under Node's built-in WASI (no dependency): `node wasi-run.mjs PROGRAM.wasm [args...]`.
// Cargo uses it as the target's runner, so a test built for WebAssembly runs as a native one does (see README.md):
//   CARGO_TARGET_WASM32_WASIP1_RUNNER="node $PWD/wasi-run.mjs" cargo test --release -p enterprise-core \
//       --target wasm32-wasip1 --test replay
//
// Reactor mode, for the core's module (`wasm/`, which has no `main`): `node wasi-run.mjs --reactor MODULE.wasm IN OUT`
// reads IN, a run of requests each as a u32 little-endian length and its bytes, calls the module's `ent_call` on each
// in turn (one instance, so one society) and writes OUT as the same framing of the responses. Exits 0. With ENT_TIMES
// set to a file, it also writes there a line of nanoseconds for each request (the binding's spike: `just binding-spike`).
// The program sees the current directory (preopened as `.`), its parent (as `/sim`: Cargo runs a crate's tests in
// its folder, sim/core, so that is sim/, where they read `wire/fixtures` and `scenarios`; a WASI program can't climb
// out of a preopen) and the environment; its exit code is the runner's.
import { readFile, writeFile } from 'node:fs/promises';
import { argv, env, exit, emitWarning, hrtime } from 'node:process';

// (Node calls WASI experimental and says so on every run: quiet that one warning.)
process.emitWarning = (warning, ...rest) =>
    String(warning).includes('WASI') ? undefined : emitWarning.call(process, warning, ...rest);
// (Imported only now: node:wasi warns as it loads, and a static import would load it before the line above.)
const { WASI } = await import('node:wasi');

const reactor = argv[2] === '--reactor';
const [program, ...args] = argv.slice(reactor ? 3 : 2);
if (!program || (reactor && args.length !== 2)) {
    console.error('usage: node wasi-run.mjs PROGRAM.wasm [args...]\n       node wasi-run.mjs --reactor MODULE.wasm IN OUT');
    exit(2);
}
const wasi = new WASI({ version: 'preview1', args: [program, ...(reactor ? [] : args)], env, returnOnExit: true, preopens: { '.': '.', '/sim': '..' } });
const module = await WebAssembly.compile(await readFile(program));
const instance = await WebAssembly.instantiate(module, wasi.getImportObject());

if (reactor) {
    wasi.initialize(instance);
    const { memory, ent_abi, ent_alloc, ent_call, ent_free } = instance.exports;
    if (ent_abi() !== 1) {
        console.error(`the module's ABI is ${ent_abi()}, not 1`);
        exit(1);
    }
    const input = await readFile(args[0]);
    const frames = [];
    const times = [];
    for (let at = 0; at < input.length; ) {
        const len = input.readUInt32LE(at);
        at += 4;
        const started = hrtime.bigint();
        const ptr = ent_alloc(len);
        new Uint8Array(memory.buffer, ptr, len).set(input.subarray(at, at + len));
        at += len;
        const packed = ent_call(ptr, len); // a BigInt: pointer << 32 | length
        const out = Number(packed >> 32n);
        const outLen = Number(packed & 0xffffffffn);
        frames.push(Buffer.from(new Uint8Array(memory.buffer, out, outLen))); // (copied before anything grows memory)
        ent_free(ptr, len);
        ent_free(out, outLen);
        times.push(hrtime.bigint() - started);
    }
    if (env.ENT_TIMES) {
        await writeFile(env.ENT_TIMES, times.map(t => `${t}\n`).join(''));
    }
    const sizes = Buffer.alloc(4 * frames.length);
    frames.forEach((f, i) => sizes.writeUInt32LE(f.length, 4 * i));
    await writeFile(args[1], Buffer.concat(frames.flatMap((f, i) => [sizes.subarray(4 * i, 4 * i + 4), f])));
    exit(0);
}
exit(wasi.start(instance));
