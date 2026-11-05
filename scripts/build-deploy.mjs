// Builds promo page + browser app for https://krocon.de/products/talk-typer/ into dist/deploy/:
//   dist/deploy/talk-typer/       promo page (site/, minified)  → https://krocon.de/products/talk-typer/
//   dist/deploy/talk-typer/app/   web app (Whisper as WASM)     → https://krocon.de/products/talk-typer/app/
// Upload the folder `dist/deploy/talk-typer` to `/products/` on the server.
// Usage: node scripts/build-deploy.mjs   (DEPLOY_BASE=/other/path/ to build for another location)
import { execSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, rmSync } from 'node:fs';

const base = process.env.DEPLOY_BASE ?? '/products/talk-typer/';
const appBuild = 'dist/talk-typer-web-deploy';
const out = 'dist/deploy/talk-typer';

const run = (cmd) => execSync(cmd, { stdio: 'inherit' });

// Web app: worker, WASM and worklet resolve relative to the base href.
run('pnpm worklets');
run('pnpm wasm');
run(`pnpm exec ng build --configuration web --base-href ${base}app/ --output-path ${appBuild}`);
run('pnpm build:site');

rmSync('dist/deploy', { recursive: true, force: true });
mkdirSync(out, { recursive: true });
cpSync('dist/talk-typer-site', out, { recursive: true });
const browser = `${appBuild}/browser`;
if (!existsSync(browser)) throw new Error(`web build missing: ${browser}`);
cpSync(browser, `${out}/app`, { recursive: true });

console.log(`deploy: ${out}/ → ${base}  ·  ${out}/app/ → ${base}app/`);
