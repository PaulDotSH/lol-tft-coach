import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Tauri loads http://localhost:5173 in dev and web/dist in release.
// Two pages: the dashboard (index.html) and the in-game overlay window (overlay.html).
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  base: './',
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    target: 'es2022',
    rolldownOptions: {
      input: {
        main: fileURLToPath(new URL('index.html', import.meta.url)),
        overlay: fileURLToPath(new URL('overlay.html', import.meta.url)),
      },
    },
  },
});
