import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'node:path';

export default defineConfig({
  plugins: [react()],
  root: path.resolve(import.meta.dirname, '../..'),
  build: { target: 'es2022', emptyOutDir: true, rollupOptions: { input: path.resolve(import.meta.dirname, 'index.html') } },
});
