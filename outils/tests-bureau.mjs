// Lance l'application de bureau avec le port de débogage de WebView2 ouvert, attend la fenêtre,
// passe les tests Playwright de tests/bureau, puis ferme tout.
//   node outils/tests-bureau.mjs            : application de développement (npx tauri dev)
//   node outils/tests-bureau.mjs <exe>      : exécutable déjà construit (CI)
import { spawn, spawnSync } from 'node:child_process'

const PORT = '9222'
const exe = process.argv[2]
const env = { ...process.env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${PORT}`, PORT_DEBOGAGE: PORT }
const application = exe
  ? spawn(exe, [], { env, stdio: 'inherit' })
  : spawn('npx', ['tauri', 'dev'], { env, stdio: 'inherit', shell: true })

let sortie = null
application.on('exit', (code, signal) => {
  sortie = { code, signal }
  console.error(`L'application s'est arrêtée (code ${code}, signal ${signal})`)
})

async function attendreFenetre(delai = 300_000) {
  const fin = Date.now() + delai
  while (Date.now() < fin) {
    if (sortie && exe) throw new Error("L'application s'est arrêtée avant d'ouvrir sa fenêtre")
    try {
      const pages = await (await fetch(`http://localhost:${PORT}/json/list`)).json()
      if (pages.some((p) => p.type === 'page')) return
    } catch {
      // pas encore prête
    }
    await new Promise((r) => setTimeout(r, 1000))
  }
  if (process.platform === 'win32') {
    // diagnostic : l'application et WebView2 tournent-ils ?
    spawnSync('tasklist', ['/fi', 'imagename eq song-audio-bureau.exe'], { stdio: 'inherit' })
    spawnSync('tasklist', ['/fi', 'imagename eq msedgewebview2.exe'], { stdio: 'inherit' })
  }
  throw new Error('La fenêtre de l’application ne s’est pas ouverte')
}

function arreter() {
  // synchrone : sinon le script se termine avant, et le serveur Vite reste ouvert
  if (process.platform === 'win32') spawnSync('taskkill', ['/pid', String(application.pid), '/t', '/f'], { stdio: 'ignore' })
  else application.kill()
}

let code = 1
try {
  await attendreFenetre()
  const tests = spawn('npx', ['playwright', 'test', '-c', 'playwright.bureau.config.ts'], { env, stdio: 'inherit', shell: true })
  code = await new Promise((r) => tests.on('exit', r))
} finally {
  arreter()
}
process.exit(code)
