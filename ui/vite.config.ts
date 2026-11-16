import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig(({ command }) => ({
  plugins: [tailwindcss(), svelte()],
  // Developer tools (the mock backend for a browser, the performance log, the seek-fill modes) are in `vite` / `tauri dev`
  // and in builds made with VITE_PYXEE_DEV_TOOLS=1 (the single-file preview is one); a plain production build is a release
  // and has none of them. A literal that the bundler substitutes, so the code behind it is dropped from a release.
  define: {
    __DEV_TOOLS__: JSON.stringify(command === 'serve' || !!process.env.SINGLE || process.env.VITE_PYXEE_DEV_TOOLS === '1'),
  },
  clearScreen: false,
  resolve: {
    alias: { $lib: fileURLToPath(new URL('./src/lib', import.meta.url)) },
    conditions: ['browser'],
  },
  server: { port: 5173, strictPort: true },
  // WebView2 is evergreen Chromium; no need to transpile further down.
  build: {
    target: 'chrome110',
    sourcemap: false,
    // SINGLE=1 builds one self-contained bundle (used for the shareable single-file preview).
    ...(process.env.SINGLE ? { outDir: 'dist-single', rollupOptions: { output: { inlineDynamicImports: true } } } : {}),
  },
  test: { environment: 'jsdom', include: ['src/**/*.test.ts'], setupFiles: ['src/test-setup.ts'] },
}));
