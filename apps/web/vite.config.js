import { defineConfig } from 'vite'
import { fileURLToPath, URL } from 'node:url'
import vue from '@vitejs/plugin-vue'
export default defineConfig({
  plugins: [vue()],
  build: {rollupOptions:{output:{onlyExplicitManualChunks:true,manualChunks(id){if(id.includes('element-plus')||id.includes('@element-plus'))return 'element';if(id.includes('node_modules')&&(id.includes('/vue')||id.includes('@vue')))return 'vue'}}}},
  server: { port: 5173, proxy: { '/v1': 'http://127.0.0.1:8787', '/healthz': 'http://127.0.0.1:8787' } },
  resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) } }
})
