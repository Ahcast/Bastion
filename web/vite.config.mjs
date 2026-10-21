import { defineConfig } from 'vite';
export default defineConfig({
  server: { host: '127.0.0.1', port: 8845, strictPort: true },
  preview: { host: '127.0.0.1', port: 8845, strictPort: true },
  build: { target: 'es2022', sourcemap: false, chunkSizeWarningLimit: 300 }
});
