// Runs a wasm32-wasip1 program under Node's built-in WASI (no dependency): `node wasi-run.mjs PROGRAM.wasm [args...]`.
// Cargo uses it as the target's runner, so a test built for WebAssembly runs as a native one does (see README.md):
//   CARGO_TARGET_WASM32_WASIP1_RUNNER="node $PWD/wasi-run.mjs" cargo test --release -p enterprise-core \
//       --target wasm32-wasip1 --test replay
// The program sees the current directory (preopened as `.`) and the environment; its exit code is the runner's.
import { readFile } from 'node:fs/promises';
import { argv, env, exit, emitWarning } from 'node:process';

// (Node calls WASI experimental and says so on every run: quiet that one warning.)
process.emitWarning = (warning, ...rest) =>
    String(warning).includes('WASI') ? undefined : emitWarning.call(process, warning, ...rest);
// (Imported only now: node:wasi warns as it loads, and a static import would load it before the line above.)
const { WASI } = await import('node:wasi');

const [program, ...args] = argv.slice(2);
if (!program) {
    console.error('usage: node wasi-run.mjs PROGRAM.wasm [args...]');
    exit(2);
}
const wasi = new WASI({ version: 'preview1', args: [program, ...args], env, returnOnExit: true, preopens: { '.': '.' } });
const module = await WebAssembly.compile(await readFile(program));
const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
exit(wasi.start(instance));
