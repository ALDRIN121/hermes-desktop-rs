import { createRequire } from 'node:module'
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { spawnSync } from 'node:child_process'
import { fileURLToPath, pathToFileURL } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const app = path.join(root, 'renderer/upstream/apps/desktop')
const output = path.join(root, 'renderer/dist')
const upstream = path.join(root, 'renderer/upstream')
if (Number(process.versions.node.split('.')[0]) < 26) {
  throw new Error('The pinned Hermes renderer requires Node >=26. Select that runtime before building.')
}
if (!existsSync(path.join(app, 'node_modules'))) {
  throw new Error('Install the pinned renderer workspace dependencies with pnpm and its frozen lockfile first. See renderer/README.md.')
}
const require = createRequire(path.join(app, 'package.json'))
const { sourceHash } = await import(pathToFileURL(path.join(upstream, 'scripts/build/freshness.mjs')).href)
const rendererSourceHash = sourceHash(upstream, 'desktop')
const vite = path.join(path.dirname(require.resolve('vite/package.json')), 'bin/vite.js')
const build = spawnSync(process.execPath, [vite, 'build', '--outDir', output, '--emptyOutDir'], {
  cwd: app, stdio: 'inherit', env: { ...process.env, NODE_ENV: 'production' }
})
if (build.status !== 0) process.exit(build.status ?? 1)
if (sourceHash(upstream, 'desktop') !== rendererSourceHash) {
  throw new Error('Renderer source changed during the build. Rebuild before packaging.')
}

// Keep shell-owned review views available while the unified renderer is integrated.
for (const name of ['task.html', 'task.js', 'task.css', 'codex.html', 'codex.js', 'codex.css']) {
  copyFileSync(path.join(root, 'ui', name), path.join(output, name))
}
const snapshot = JSON.parse(readFileSync(path.join(root, 'renderer/upstream/UPSTREAM_SNAPSHOT.json'), 'utf8'))
mkdirSync(output, { recursive: true })
writeFileSync(path.join(output, 'renderer-build.json'), JSON.stringify({
  upstreamRevision: snapshot.revision,
  upstreamArchiveSha256: snapshot.archiveSha256,
  upstreamDesktopSourceHash: snapshot.upstreamDesktopSourceHash ?? null,
  rendererSourceHash,
  builtAt: new Date().toISOString(),
  host: 'tauri-rust'
}, null, 2) + '\n')
console.log(`Hermes renderer built at ${output}`)
