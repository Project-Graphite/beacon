import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

const here = (path: string) => fileURLToPath(new URL(path, import.meta.url));

export default defineConfig({
  root: here('..'),
  plugins: [
    react(),
    tailwindcss(),
    {
      name: 'beacon-preview',
      transformIndexHtml: (html) =>
        html.replace(
          '<div id="root"></div>',
          `<script>window.__BEACON_INITIAL__ = { snapshot: null, ui: {} };</script>
    <link rel="stylesheet" href="/preview/stage.css" />
    <div id="root"></div>`,
        ),
    },
  ],
  resolve: {
    alias: {
      '@tauri-apps/api/core': here('./tauri-core.ts'),
      '@tauri-apps/api/event': here('./tauri-event.ts'),
    },
  },
  build: {
    rollupOptions: { input: { index: here('../index.html'), demo: here('./demo.html') } },
  },
  clearScreen: false,
  server: { port: 5174, strictPort: true },
});
