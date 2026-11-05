// Promo page (site/) – a static page without dependencies.
//   node scripts/build-site.mjs           → dist/talk-typer-site/ (inline <style>/<script> minified with esbuild)
//   node scripts/build-site.mjs --serve   → serves site/ unchanged on http://localhost:1450 (PORT=… to change)
import { transform } from 'esbuild';
import {
  cpSync,
  createReadStream,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { createServer } from 'node:http';
import { extname, join, normalize, resolve } from 'node:path';

const srcDir = resolve('site');
const outDir = resolve('dist/talk-typer-site');

if (process.argv.includes('--serve')) serve();
else await build();

async function build() {
  rmSync(outDir, { recursive: true, force: true });
  mkdirSync(outDir, { recursive: true });
  // everything except the page itself is copied as is (images, icons, …)
  cpSync(srcDir, outDir, { recursive: true, filter: (p) => !p.endsWith('.html') });

  for (const page of ['index.html']) {
    const html = readFileSync(join(srcDir, page), 'utf8');
    const out = await replaceAsync(
      html,
      /<(style|script)>([\s\S]*?)<\/\1>/g,
      async (_, tag, code) => {
        const { code: min } = await transform(code, {
          loader: tag === 'style' ? 'css' : 'js',
          minify: true,
          target: tag === 'style' ? ['chrome111', 'firefox113', 'safari16.4'] : 'es2022',
          legalComments: 'none',
        });
        return `<${tag}>${min.trim()}</${tag}>`;
      },
    );
    writeFileSync(join(outDir, page), out);
    console.log(`site: ${page} ${kb(html)} → ${kb(out)} → ${outDir}`);
  }
}

function serve() {
  const port = Number(process.env.PORT) || 1450;
  const types = {
    '.html': 'text/html; charset=utf-8',
    '.css': 'text/css',
    '.js': 'text/javascript',
    '.svg': 'image/svg+xml',
    '.png': 'image/png',
    '.jpg': 'image/jpeg',
    '.webp': 'image/webp',
    '.ico': 'image/x-icon',
  };
  createServer((req, res) => {
    let file = normalize(join(srcDir, decodeURIComponent(new URL(req.url, 'http://x').pathname)));
    if (!file.startsWith(srcDir)) return res.writeHead(403).end();
    if (existsSync(file) && statSync(file).isDirectory()) file = join(file, 'index.html');
    if (!existsSync(file)) return res.writeHead(404).end('not found');
    res.writeHead(200, {
      'content-type': types[extname(file)] ?? 'application/octet-stream',
      'cache-control': 'no-store',
    });
    createReadStream(file).pipe(res);
  }).listen(port, () => console.log(`site: http://localhost:${port}`));
}

async function replaceAsync(str, re, fn) {
  const parts = await Promise.all([...str.matchAll(re)].map((m) => fn(...m)));
  let i = 0;
  return str.replace(re, () => parts[i++]);
}

function kb(s) {
  return `${(Buffer.byteLength(s) / 1024).toFixed(1)} KB`;
}
