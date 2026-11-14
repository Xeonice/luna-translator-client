import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig(async () => {
  const { generalVisualAssets, generalVisualBuildInputs } = await import(new URL('../../scripts/report/general_visual_assets.mjs', import.meta.url).href);
  return {
    plugins: [react(), generalVisualAssets()],
    clearScreen: false,
    server: { port: 1420, strictPort: true },
    build: {
      target: 'es2022',
      rollupOptions: {
        input: generalVisualBuildInputs(),
        output: {
          entryFileNames: (chunk: { name: string }) => chunk.name === 'general-reference-replay' ? 'general-reference/replay.js' : 'assets/[name]-[hash].js',
        },
      },
    },
  };
});
