import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import App from './App'
import { GlobalTooltip } from './ui/GlobalTooltip'
import { installErrorLogging } from './lib/log'
import { installWindowDrag } from './lib/window-drag'
import { installEdgeResize } from './lib/window'
import './styles.css'

document.documentElement.dataset.window = 'main'
if (/Linux/i.test(navigator.userAgent) && !/Android/i.test(navigator.userAgent)) document.documentElement.dataset.os = 'linux'
installErrorLogging('main')
installWindowDrag()
installEdgeResize()
// Keep the app's own menus; never the browser's Back/Reload menu.
document.addEventListener('contextmenu', (e) => {
  if (!(e.target instanceof HTMLElement && e.target.closest('input, textarea, .kryo-log'))) e.preventDefault()
})

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
    <GlobalTooltip />
  </StrictMode>,
)
