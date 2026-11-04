// Bundles the AudioWorklet module(s) (frontend/src/app/stt/*.worklet.ts) to
// frontend/public/worklets/*.js. The Angular application builder does not bundle AudioWorklet
// modules, and Blob URLs would need a relaxed CSP – so they are emitted as same-origin assets.
// Taken from sonic-lab (scripts/build-worklets.mjs).
// Usage: node scripts/build-worklets.mjs [--watch]
import { build, context } from 'esbuild';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';

const srcDir = 'frontend/src/app/stt';
const entryPoints = Object.fromEntries(
  readdirSync(srcDir)
    .filter((f) => f.endsWith('.worklet.ts'))
    .map((f) => [f.replace(/\.worklet\.ts$/, ''), join(srcDir, f)]),
);

const options = {
  entryPoints,
  outdir: 'frontend/public/worklets',
  bundle: true,
  format: 'esm',
  target: 'es2022',
  minify: true,
  sourcemap: false,
  legalComments: 'none',
  logLevel: 'info',
};

if (process.argv.includes('--watch')) {
  const ctx = await context(options);
  await ctx.watch();
} else {
  await build(options);
}
