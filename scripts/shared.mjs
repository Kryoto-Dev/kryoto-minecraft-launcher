#!/usr/bin/env node
/**
 * The front-end files Kryoto Minecraft Launcher shares with Kryoto Desktop.
 *
 * Desktop is the source of truth. The launcher keeps committed copies (so it builds
 * on its own, offline) of the files listed in shared.json, and this script
 * stops them drifting:
 *
 *   node scripts/shared.mjs check            Offline. Fails if a copy was edited here,
 *                                            or is missing. Runs as part of `pnpm build`.
 *   node scripts/shared.mjs check --remote   Also says which copies are behind Desktop's
 *                                            main (a warning, never a failure).
 *   node scripts/shared.mjs sync             Copy the listed files from Desktop's
 *                                            committed HEAD and re-lock the hashes.
 *     --ref <rev>      a commit, tag or branch of Desktop instead of HEAD
 *     --from <dir>     a local Desktop checkout (default ../kryoto-desktop, else a
 *                      shallow clone of the GitHub repo)
 *     --worktree a,b   copy these paths from Desktop's working tree, uncommitted edits
 *                      included (for before the Desktop change is committed; re-sync after)
 *
 * Fix a shared file in kryoto-desktop, then `pnpm sync:shared` here. A
 * difference that belongs to the launcher alone goes in a launcher-only file instead.
 * Hashes ignore line endings (the working trees are CRLF, the repos LF).
 */
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const lockPath = join(root, 'shared.json')

const args = process.argv.slice(2)
const command = args[0]
const flag = (name) => args.includes(name)
const option = (name) => {
  const at = args.indexOf(name)
  return at === -1 ? null : (args[at + 1] ?? null)
}

const lf = (text) => text.replace(/\r\n/g, '\n')
const sha256 = (text) => createHash('sha256').update(lf(text)).digest('hex')
const git = (cwd, ...gitArgs) =>
  execFileSync('git', gitArgs, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim()

/** A committed file's exact text (`git()` trims, which would eat the final newline). */
const show = (cwd, spec) =>
  execFileSync('git', ['show', spec], { cwd, encoding: 'utf8', maxBuffer: 1 << 26, stdio: ['ignore', 'pipe', 'pipe'] })

function readLock() {
  return JSON.parse(readFileSync(lockPath, 'utf8'))
}

function fail(message) {
  console.error(message)
  process.exit(1)
}

/** A shallow clone of Desktop, kept inside the repo (the PC's temp folder blocks repo programs). */
function cloneDesktop(url, ref) {
  const dir = join(root, '.tmp', 'shared-desktop')
  rmSync(dir, { recursive: true, force: true })
  mkdirSync(dir, { recursive: true })
  git(dir, 'init', '-q')
  git(dir, 'fetch', '-q', '--depth', '1', url, ref)
  git(dir, 'checkout', '-q', 'FETCH_HEAD')
  return dir
}

function check() {
  const lock = readLock()
  const bad = []
  for (const path of lock.paths) {
    const file = join(root, 'src', path)
    if (!existsSync(file)) {
      bad.push(`${path}: missing`)
      continue
    }
    if (sha256(readFileSync(file, 'utf8')) !== lock.hashes[path]) bad.push(`${path}: differs from the locked copy`)
  }
  if (bad.length) {
    console.error('Shared files do not match shared.json:')
    for (const line of bad) console.error(`  ${line}`)
    console.error(
      'These are copies of kryoto-desktop files. Make the change in kryoto-desktop and run `pnpm sync:shared`;\n' +
        'if the difference belongs to the launcher alone, keep it in a launcher-only file.',
    )
    process.exit(1)
  }
  console.log(`Shared files: ${lock.paths.length} match shared.json (Desktop ${lock.ref.slice(0, 9)}${lock.worktree.length ? ', plus uncommitted ' + lock.worktree.join(', ') : ''}).`)
}

function checkRemote() {
  const lock = readLock()
  const dir = cloneDesktop(lock.source, 'main')
  try {
    const main = git(dir, 'rev-parse', 'HEAD')
    const behind = []
    for (const path of lock.paths) {
      const theirs = show(dir, `HEAD:src/${path}`)
      if (sha256(theirs) !== lock.hashes[path]) behind.push(path)
    }
    if (!behind.length) return console.log(`The launcher's shared files are current with Desktop main (${main.slice(0, 9)}).`)
    console.log(`::warning::Behind Desktop main (${main.slice(0, 9)}): ${behind.join(', ')}. Run \`pnpm sync:shared --ref main\` when ready.`)
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
}

function sync() {
  const lock = readLock()
  const worktree = (option('--worktree') ?? '').split(',').filter(Boolean)
  let dir = option('--from')
  let cloned = false
  if (!dir) {
    const sibling = resolve(root, '..', 'kryoto-desktop')
    if (existsSync(join(sibling, '.git'))) dir = sibling
  }
  if (!dir) {
    dir = cloneDesktop(lock.source, option('--ref') ?? 'main')
    cloned = true
  }
  dir = resolve(dir)
  try {
    const ref = git(dir, 'rev-parse', option('--ref') ?? 'HEAD')
    const hashes = {}
    const changed = []
    for (const path of lock.paths) {
      const text = lf(worktree.includes(path) ? readFileSync(join(dir, 'src', path), 'utf8') : show(dir, `${ref}:src/${path}`))
      const file = join(root, 'src', path)
      const before = existsSync(file) ? sha256(readFileSync(file, 'utf8')) : null
      if (before !== sha256(text)) {
        mkdirSync(dirname(file), { recursive: true })
        writeFileSync(file, text)
        changed.push(path)
      }
      hashes[path] = sha256(text)
    }
    writeFileSync(lockPath, JSON.stringify({ ...lock, ref, worktree, hashes }, null, 2) + '\n')
    console.log(`Synced from Desktop ${ref.slice(0, 9)}${worktree.length ? ` (uncommitted in Desktop: ${worktree.join(', ')})` : ''}: ${changed.length ? changed.join(', ') : 'nothing changed'}.`)
  } finally {
    if (cloned) rmSync(dir, { recursive: true, force: true })
  }
}

if (command === 'check') {
  check()
  if (flag('--remote')) checkRemote()
} else if (command === 'sync') {
  sync()
} else {
  fail('Usage: node scripts/shared.mjs <check [--remote] | sync [--ref <rev>] [--from <dir>] [--worktree a,b]>')
}
