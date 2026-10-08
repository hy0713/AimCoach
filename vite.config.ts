import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
export default defineConfig({plugins:[vue()],server:{port:15420,strictPort:true},clearScreen:false,build:{rollupOptions:{output:{manualChunks:{charts:['echarts'],ui:['element-plus'],framework:['vue','pinia']}}}}});
