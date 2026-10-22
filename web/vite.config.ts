import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Tauri loads http://localhost:5173 in dev and web/dist in release.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  base: './',
  build: { outDir: 'dist', emptyOutDir: true, target: 'es2022' },
});
