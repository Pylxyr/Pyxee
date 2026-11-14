// Inlines the SINGLE=1 build into one HTML file: `npm run preview:single`.
import { readFileSync, writeFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const dir = new URL('../dist-single/', import.meta.url).pathname;
let html = readFileSync(join(dir, 'index.html'), 'utf8');
const assets = readdirSync(join(dir, 'assets'));
const css = assets.filter((f) => f.endsWith('.css')).map((f) => readFileSync(join(dir, 'assets', f), 'utf8')).join('\n');
const js = assets.filter((f) => f.endsWith('.js'));
if (js.length !== 1) throw new Error(`expected one JS bundle, found ${js.length}: ${js.join(', ')}`);
const code = readFileSync(join(dir, 'assets', js[0]), 'utf8').replaceAll('</script', '<\\/script');
html = html
  .replace(/<link rel="stylesheet"[^>]*>/g, '')
  .replace(/<script type="module"[^>]*><\/script>/g, '')
  .replace('</head>', `<style>${css}</style>\n</head>`)
  .replace('</body>', `<script type="module">${code}</script>\n</body>`);
writeFileSync(join(dir, 'pyxee-preview.html'), html);
console.log(`wrote dist-single/pyxee-preview.html (${(html.length / 1024).toFixed(0)} KB)`);
