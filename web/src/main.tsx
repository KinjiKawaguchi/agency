import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './app/App'
import { registerServiceWorker } from './pwa/register'
import './theme/tokens.css'
import './theme/base.css'

registerServiceWorker()

const root = document.getElementById('root')
if (root === null) throw new Error('Missing #root element')
createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
