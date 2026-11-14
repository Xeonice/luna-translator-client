import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'node:path';

const directory = import.meta.dirname;
const desktop = path.resolve(directory, '../..');
const repository = path.resolve(desktop, '../..');
export default defineConfig({
  root: directory,
  publicDir: false,
  plugins: [react(), { name: 'luna-live-review-health', configureServer(server) {
    server.middlewares.use('/__luna_live_preview_health', (_request, response) => {
      response.setHeader('Content-Type', 'application/json');
      response.end(JSON.stringify({ kind: 'luna-live-ui-preview-v1', pid: process.pid, repository, native: false, data: 'controlled-memory-only' }));
    });
  } }],
  cacheDir: path.join(repository, '.build/live-ui-preview/vite-cache'),
  server: { host: '127.0.0.1', port: 1421, strictPort: true, cors: false,
    fs: { strict: true, allow: [directory, path.join(desktop, 'src'), path.join(desktop, 'tests/support'), path.join(desktop, 'node_modules'), path.join(repository, 'node_modules'), path.join(repository, 'docs/design/first-product-v1/assets')] } },
  build: { outDir: path.join(repository, '.build/live-ui-preview/bundle'), emptyOutDir: true,
    rollupOptions: { input: { review: path.join(directory, 'index.html'), client: path.join(directory, 'client.html') } } },
});
