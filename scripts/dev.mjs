// Run the app against the Vite dev server: `pnpm app:dev`.
// Everything after the script name goes to `tauri dev` as written.

import { spawnSync } from 'node:child_process'
import { root, tauriEnv } from './tauri-env.mjs'

const run = spawnSync(['pnpm tauri dev', ...process.argv.slice(2)].join(' '), {
  cwd: root,
  env: tauriEnv(),
  stdio: 'inherit',
  shell: true,
})
process.exit(run.status ?? 1)
