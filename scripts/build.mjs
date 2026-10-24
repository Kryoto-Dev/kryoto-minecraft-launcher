// Build the installer without the build machine in it (Kryoto Desktop's
// scripts/build.mjs, without the C-library clean-up this app has no need for:
// every dependency is Rust, TLS included).
//
//   pnpm app:build            the release build, signed for the updater with
//                             TAURI_SIGNING_PRIVATE_KEY (tauri asks for its
//                             password when TAURI_SIGNING_PRIVATE_KEY_PASSWORD
//                             is not set).
//   pnpm app:build:local      the same build for trying on this PC: unsigned,
//                             no updater files, and the installer is copied out
//                             of the repository (see below).
//
// Rust keeps the source path of every dependency in the binary for its panic
// messages, which names the account that built it. Those paths are remapped
// for the build, and the build fails if the home folder is still in the binary.

import { spawnSync } from 'node:child_process'
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { tauriEnv, root } from './tauri-env.mjs'

const home = os.homedir()
const cargoHome = process.env.CARGO_HOME || path.join(home, '.cargo')
const rustupHome = process.env.RUSTUP_HOME || path.join(home, '.rustup')

const remaps = [
  [cargoHome, '/cargo'],
  [rustupHome, '/rustup'],
  [root, '/kryoto-minecraft-launcher'],
  [home, '/home'],
]
// CARGO_ENCODED_RUSTFLAGS (separated by 0x1f), not RUSTFLAGS: RUSTFLAGS is
// split on spaces, and a checkout under "OneDrive - Company" has them.
const flags = remaps.map(([from, to]) => `--remap-path-prefix=${from}=${to}`)
const inherited = process.env.CARGO_ENCODED_RUSTFLAGS
  ? process.env.CARGO_ENCODED_RUSTFLAGS.split('\x1f')
  : (process.env.RUSTFLAGS ?? '').split(' ').filter(Boolean)
const env = tauriEnv({ CARGO_ENCODED_RUSTFLAGS: [...inherited, ...flags].join('\x1f') })
delete env.RUSTFLAGS

const local = process.argv.includes('--local')
if (local) {
  for (const name of Object.keys(env)) if (name.startsWith('TAURI_SIGNING_PRIVATE_KEY')) delete env[name]
  mkdirSync(path.join(root, '.tmp'), { recursive: true })
  writeFileSync(path.join(root, '.tmp', 'local.conf.json'), JSON.stringify({ bundle: { createUpdaterArtifacts: false } }))
}

const extra = [...process.argv.slice(2).filter((a) => a !== '--local'), ...(local ? ['--config', '.tmp/local.conf.json'] : [])]
const tauri = (args) => spawnSync(['pnpm tauri', ...args, ...extra].join(' '), { cwd: root, env, stdio: 'inherit', shell: true })
const built = tauri(['build', '--no-bundle'])
if (built.status !== 0) process.exit(built.status ?? 1)

const target = process.env.CARGO_TARGET_DIR || path.join(root, 'src-tauri', 'target')
const exe = path.join(target, 'release', process.platform === 'win32' ? 'kryoto-minecraft-launcher.exe' : 'kryoto-minecraft-launcher')

const bytes = readFileSync(exe).toString('latin1').toLowerCase()
const name = os.userInfo().username.toLowerCase()
const leaks = [home.toLowerCase(), `users\\${name}`, `users/${name}`, `home/${name}`]
  .flatMap((s) => [`${s}/`, `${s}\\`])
  .filter((s) => bytes.includes(s))
if (leaks.length) {
  console.error(`\nThe build still contains ${leaks.map((l) => JSON.stringify(l)).join(' and ')}. Do not ship it.`)
  process.exit(1)
}
console.log('\nNo build-machine paths in the binary.')

const bundled = tauri(['bundle'])
if (bundled.status !== 0) process.exit(bundled.status ?? 1)

// A program started from inside the repository folder can be confined to
// writing inside it, so the local installer is copied out to be run from there.
if (local) {
  const dir = path.join(target, 'release', 'bundle', process.platform === 'win32' ? 'nsis' : 'appimage')
  const ending = process.platform === 'win32' ? '.exe' : '.AppImage'
  const found = existsSync(dir) ? readdirSync(dir).filter((f) => f.endsWith(ending)) : []
  if (found.length === 0) {
    console.error(`\nNo installer found in ${dir}.`)
    process.exit(1)
  }
  const out =
    process.env.KRYOTO_LOCAL_OUT ||
    (process.platform === 'win32' && existsSync('D:/') ? 'D:/kryo-builds/kryoto-minecraft-launcher' : path.join(home, 'kryoto-builds', 'kryoto-minecraft-launcher'))
  mkdirSync(out, { recursive: true })
  for (const file of found) copyFileSync(path.join(dir, file), path.join(out, file))
  console.log(`\nLocal build ready (unsigned): ${path.join(out, found[0])}\nRun it from there, not from the repository.`)
}
