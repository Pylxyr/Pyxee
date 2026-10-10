import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  plugins: [tailwindcss(), svelte()],
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
});
