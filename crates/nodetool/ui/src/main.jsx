import { createRoot } from 'react-dom/client'
import { ReactFlowProvider } from '@xyflow/react'
import { Editor } from './editor.jsx'
import '@xyflow/react/dist/style.css'
import './style.css'

createRoot(document.querySelector('#root')).render(
  <ReactFlowProvider>
    <Editor />
  </ReactFlowProvider>,
)
