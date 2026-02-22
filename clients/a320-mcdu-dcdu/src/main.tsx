/**
 * main.tsx — Application entry point.
 *
 * Mounts the React root and imports the global A320 cockpit stylesheet.
 */
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './styles/index.css'
import App from './App'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
