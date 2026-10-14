// Dev-only: regenerate lossless GitHub headers and the desktop PNG from vector truth.
// Requires the same Chromium / puppeteer-core setup as verify-ui.cjs.
const fs = require('fs');
const path = require('path');
let puppeteer;
try { puppeteer = require('puppeteer-core'); } catch { puppeteer = require('/tmp/test-runner/node_modules/puppeteer-core'); }
const root = path.resolve(__dirname, '../..');
const { execFileSync } = require('child_process');
(async () => {
  const executablePath = [process.env.CHROME_BIN, '/home/dev/.cache/puppeteer/chrome/linux-152.0.7977.54/chrome-linux64/chrome', '/usr/bin/chromium', '/usr/bin/google-chrome'].find(p => p && fs.existsSync(p));
  if (!executablePath) throw new Error('Set CHROME_BIN to a Chromium executable');
  const browser = await puppeteer.launch({ executablePath, headless: true, args: ['--no-sandbox'] });
  try {
    const page = await browser.newPage();
    await page.setViewport({ width: 1100, height: 1100, deviceScaleFactor: 1 });
    const icon = fs.readFileSync(path.join(root, 'docs/media/brand/pingloom-app-icon.svg'), 'utf8');
    await page.setContent(`<style>body{margin:0}svg{display:block;width:1024px;height:1024px}</style>${icon}`);
    await (await page.$('svg')).screenshot({ path: path.join(root, 'icons/icon.png'), omitBackground: true });
    const font = fs.readFileSync(path.join(root, 'ui/src/fonts/plus-jakarta-sans-latin-wght-normal.woff2')).toString('base64');
    for (const [mode, color] of [['light', '#202330'], ['dark', '#F5F6FC']]) {
      await page.setContent(`<style>@font-face{font-family:Jakarta;font-weight:200 800;font-style:normal;src:url(data:font/woff2;base64,${font})}body{margin:0}.lockup{width:max-content;height:96px;display:flex;align-items:center;gap:20px;color:${color};font-synthesis:none;font:750 54px Jakarta;letter-spacing:-1.8px}.lockup svg{width:80px;height:80px;flex:none}</style><div class="lockup">${icon}<span>Pingloom</span></div>`);
      await page.evaluate(() => document.fonts.ready);
      await page.setViewport({ width: 480, height: 120, deviceScaleFactor: 2 });
      await (await page.$('.lockup')).screenshot({ path: path.join(root, `docs/media/brand/pingloom-wordmark-${mode}.png`), omitBackground: true });
    }
  } finally { await browser.close(); }
  const output = fs.mkdtempSync(path.join(require('os').tmpdir(), 'pingloom-icons-'));
  try {
    execFileSync(process.platform === 'win32' ? 'npx.cmd' : 'npx', ['--no-install', 'tauri', 'icon', path.join(root, 'icons/icon.png'), '--output', output], { cwd: path.join(root, 'ui'), stdio: 'inherit' });
    for (const name of ['icon.ico', 'icon.icns']) fs.copyFileSync(path.join(output, name), path.join(root, 'icons', name));
  } finally { fs.rmSync(output, { recursive: true }); }
})().catch(e => { console.error(e); process.exitCode = 1; });
