// Rebuild the native V7 label atlas using the approved, bundled OFL font.
// Build-time only: npm install --prefix /tmp/test-runner puppeteer-core
// node tools/render-wheel-labels.cjs /path/to/chromium
const fs = require('node:fs');
const path = require('node:path');
let puppeteer;
try { puppeteer = require('puppeteer-core'); }
catch { puppeteer = require('/tmp/test-runner/node_modules/puppeteer-core'); }
const root = path.resolve(__dirname, '..');
const font = fs.readFileSync(path.join(root, 'ui/src/fonts/bricolage-grotesque-latin-wght-normal.woff2')).toString('base64');
const output = path.join(root, 'assets/textures/pingloom-wheel-labels.png');
(async () => {
  const browser = await puppeteer.launch({ executablePath: process.argv[2], headless: true, args: ['--no-sandbox'] });
  try {
    const page = await browser.newPage();
    await page.setViewport({ width: 460, height: 460, deviceScaleFactor: 2 });
    const directions = ['N', 'NE', 'E', 'SE', 'S', 'SW', 'W', 'NW'];
    await page.setContent(`<style>
      @font-face{font-family:Wheel;src:url(data:font/woff2;base64,${font})}
      body{margin:0;background:transparent;font-family:Wheel,sans-serif;color:#bdb1b4}
      span{position:absolute;transform:translate(-50%,-50%);white-space:nowrap;font-size:12px;font-weight:800;line-height:1}
      .release{left:230px;top:222px;font-size:10px;letter-spacing:2px}
      .cancel{left:230px;top:241px;font-size:13px;font-weight:600}
      </style>${directions.map((label, i) => `<span style="left:${230 + 204 * Math.sin(i * Math.PI / 4)}px;top:${230 - 204 * Math.cos(i * Math.PI / 4)}px">${label}</span>`).join('')}
      <span class="release">RELEASE</span><span class="cancel">to cancel</span>`);
    await page.evaluate(() => document.fonts.ready);
    await page.screenshot({ path: output, omitBackground: true });
    console.log(output);
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
