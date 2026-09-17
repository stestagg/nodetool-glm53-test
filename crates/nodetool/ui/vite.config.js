import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// The built files land in dist/ under the fixed names the server embeds
// and serves: /, /editor.js, /editor.css.
export default defineConfig({
  plugins: [react()],
  build: {
    rollupOptions: {
      output: {
        entryFileNames: 'editor.js',
        chunkFileNames: 'editor.js',
        assetFileNames: 'editor[extname]',
      },
    },
  },
})
