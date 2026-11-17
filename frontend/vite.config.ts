import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// API_PROXY_TARGET lets scripts/run_e2e.sh point the preview at its throwaway backend.
const apiProxyTarget = process.env.API_PROXY_TARGET || 'http://127.0.0.1:8082';

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    port: 8081,
    host: true,
    proxy: {
      '/api': {
        target: apiProxyTarget,
        changeOrigin: true,
      },
    },
  },
  preview: {
    port: 8081,
    host: true,
    proxy: {
      '/api': {
        target: apiProxyTarget,
        changeOrigin: true,
      },
    },
  },
});
