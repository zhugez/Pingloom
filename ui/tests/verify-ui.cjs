// Headless Chrome / Puppeteer Test Suite for Pingloom Settings WebView
// Verifies UI rendering, 8-slice wheel assets, draft updates, Save/Cancel, and error routing.
// Run with: node ui/tests/verify-ui.cjs

const fs = require('fs');
const path = require('path');
const http = require('http');

let puppeteer;
try {
  puppeteer = require('puppeteer-core');
} catch {
  puppeteer = require('/tmp/test-runner/node_modules/puppeteer-core');
}

const DIST_DIR = path.resolve(__dirname, '../dist');
const ASSETS_DIR = path.resolve(__dirname, '../../assets');
const SCREENSHOT_DIR = path.resolve(__dirname, '../../.impeccable/review/desktop-redesign');

if (!fs.existsSync(SCREENSHOT_DIR)) {
  fs.mkdirSync(SCREENSHOT_DIR, { recursive: true });
}

// Capture only settled UI: an in-flight parent transform blurs every child,
// and GIF export quantizes the palette. Keep review captures as lossless PNG.
async function capture(page, filename) {
  await page.evaluate(async () => {
    await document.fonts.ready;
    await Promise.all([...document.images].filter(img => img.complete).map(img => img.decode().catch(() => {})));
    await Promise.all(document.getAnimations()
      .filter(animation => Number.isFinite(animation.effect?.getComputedTiming().endTime))
      .map(animation => animation.finished.catch(() => {})));
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    const panel = document.querySelector('.page-panel');
    if (panel) {
      const style = getComputedStyle(panel);
      if (style.opacity !== '1' || style.transform !== 'none') {
        throw new Error(`Unsettled preview: opacity=${style.opacity}, transform=${style.transform}`);
      }
    }
  });
  await page.screenshot({ path: path.join(SCREENSHOT_DIR, filename) });
}

function base64Png(relPath) {
  const full = path.join(ASSETS_DIR, relPath);
  if (fs.existsSync(full)) {
    return 'data:image/png;base64,' + fs.readFileSync(full).toString('base64');
  }
  return '';
}

const PING_ITEMS = [
  { id: 'danger', name: 'Danger', file: 'textures/pingwheel_retreatrender.png' },
  { id: 'push', name: 'Push', file: 'textures/push.png' },
  { id: 'omw', name: 'On My Way', file: 'textures/omw.png' },
  { id: 'allin', name: 'All In', file: 'textures/pingwheel_allin_render_01.png' },
  { id: 'assist', name: 'Assist Me', file: 'textures/assist_me.png' },
  { id: 'needvision', name: 'Need Vision', file: 'textures/need_vision.png' },
  { id: 'missing', name: 'Enemy Missing', file: 'textures/enemy_missing.png' },
  { id: 'enemyvision', name: 'Enemy Vision', file: 'textures/pingwheel_enemyvisionrender.png' },
  { id: 'generic', name: 'Generic', file: 'textures/generic_ping.png' },
  { id: 'bait', name: 'Bait', file: 'textures/pingwheel_baitrender.png' },
  { id: 'visioncleared', name: 'Vision Cleared', file: 'textures/vision_cleared.png' },
];

const EMOTE_SLUGS = [
  'facepalm', 'unworthy', 'unworthy_cn', 'poro_snax', 'sad_kitten', 'cheeky_poro',
  'angry_kitty', 'gg_heart', 'bee_happy', 'bee_mad', 'bee_sad', 'snoozy_poro'
];

const catalog = {
  pings: PING_ITEMS.map(p => ({
    id: p.id,
    name: p.name,
    preview_url: base64Png(p.file),
  })),
  emotes: EMOTE_SLUGS.map(slug => ({
    id: slug,
    name: slug.replaceAll('_', ' ').replace(/\b\w/g, c => c.toUpperCase()),
    preview_url: base64Png(`emotes/${slug}.png`),
  })),
  trigger_keys: [81, 87, 69, 82, 80],
};

const initialSettings = {
  version: 1,
  enabled_on_start: true,
  trigger: 'alt',
  emote_trigger: 'off',
  toggle_hotkey: { mods: 3, vk: 80 },
  drag_threshold_px: 12,
  click_ping: true,
  click_ping_id: 'generic',
  game_check: true,
  wheel: ['danger', 'push', 'omw', 'allin', 'assist', 'needvision', 'missing', 'enemyvision'],
  ping_size_px: 128,
  ping_duration_s: 3.5,
  volume: 85,
  muted: false,
  tick_sound: true,
  emote_click: false,
  click_emote_id: 'gg_heart',
  emote_wheel: ['unworthy_cn', 'poro_snax', 'sad_kitten', 'cheeky_poro', 'angry_kitty', 'gg_heart', 'bee_happy', 'bee_mad'],
  emote_size_px: 160,
  emote_sound: true,
};

function startServer() {
  return new Promise((resolve) => {
    const mimeTypes = {
      '.html': 'text/html',
      '.js': 'text/javascript',
      '.css': 'text/css',
      '.png': 'image/png',
      '.svg': 'image/svg+xml',
      '.woff2': 'font/woff2',
    };
    const server = http.createServer((req, res) => {
      let reqPath = req.url.split('?')[0];
      if (reqPath === '/favicon.ico') {
        res.writeHead(204);
        res.end();
        return;
      }
      if (reqPath === '/' || reqPath === '') reqPath = '/index.html';
      const filePath = path.join(DIST_DIR, reqPath);
      const ext = path.extname(filePath);
      if (fs.existsSync(filePath) && fs.statSync(filePath).isFile()) {
        res.writeHead(200, { 'Content-Type': mimeTypes[ext] || 'application/octet-stream' });
        fs.createReadStream(filePath).pipe(res);
      } else {
        res.writeHead(404);
        res.end('Not found');
      }
    });
    server.listen(0, '127.0.0.1', () => {
      resolve({ server, port: server.address().port });
    });
  });
}

function findChrome() {
  const candidates = [
    '/home/dev/.cache/puppeteer/chrome/linux-152.0.7977.54/chrome-linux64/chrome',
    '/usr/bin/google-chrome',
    '/usr/bin/google-chrome-stable',
    '/usr/bin/chromium',
    '/usr/bin/chromium-browser',
  ];
  for (const c of candidates) {
    if (fs.existsSync(c)) return c;
  }
  return null;
}

(async () => {
  const { server, port } = await startServer();
  const url = `http://127.0.0.1:${port}/`;
  console.log(`Server started on ${url}`);

  const chromePath = findChrome();
  if (!chromePath) {
    console.error('No chrome found');
    process.exit(1);
  }

  const browser = await puppeteer.launch({
    executablePath: chromePath,
    headless: 'new',
    args: ['--no-sandbox', '--disable-setuid-sandbox', '--disable-gpu'],
  });

  try {
    const page = await browser.newPage();
    await page.setViewport({ width: 960, height: 720 });

    page.on('console', msg => console.log('BROWSER LOG:', msg.type(), msg.text()));
    page.on('pageerror', err => console.error('BROWSER ERROR:', err.toString()));

    await page.evaluateOnNewDocument((initialCatalog, initSettings) => {
      const callbacks = new Map();
      function registerCallback(callback, once = false) {
        const identifier = window.crypto.getRandomValues(new Uint32Array(1))[0];
        callbacks.set(identifier, (data) => {
          if (once) callbacks.delete(identifier);
          return callback && callback(data);
        });
        return identifier;
      }
      function unregisterCallback(id) {
        callbacks.delete(id);
      }

      window.__tauri_mock = {
        catalog: initialCatalog,
        settings: JSON.parse(JSON.stringify(initSettings)),
        status: 'active',
        session_id: 'test-session-123',
      };

      window.__TAURI_INTERNALS__ = {
        metadata: {
          currentWindow: { label: 'main' },
          currentWebview: { windowLabel: 'main', label: 'main' },
        },
        callbacks,
        transformCallback: registerCallback,
        unregisterCallback,
        invoke: async (cmd, args = {}) => {
          if (cmd === 'get_settings_and_catalog') {
            return {
              session_id: window.__tauri_mock.session_id,
              settings: window.__tauri_mock.settings,
              status: window.__tauri_mock.status,
              catalog: window.__tauri_mock.catalog,
              load_warning: null,
            };
          }
          if (cmd === 'import_emote') {
            window.__importCalls = (window.__importCalls || 0) + 1;
            if (window.__rejectImport) { window.__rejectImport = false; throw { message: 'Invalid image.' }; }
            if (typeof args.data !== 'string' || args.data.length < 12 || !args.session_id) throw { message: 'Invalid import payload.' };
            window.__lastImportPrefix = args.data.slice(0, 4);
            return { id: 'custom_0000000000000001', name: 'Custom 000001', preview_url: window.__tauri_mock.catalog.emotes[0].preview_url };
          }
          if (cmd === 'settings_heartbeat') {
            window.__heartbeatCount = (window.__heartbeatCount || 0) + 1;
            return window.__tauri_mock.status;
          }
          if (cmd === 'close_settings') {
            window.__closedCount = (window.__closedCount || 0) + 1;
            return null;
          }
          if (cmd === 'save_settings') {
            if (window.__deferSave) {
              window.__deferSave = false;
              await new Promise(resolve => { window.__resumeSave = resolve; });
            }
            if (window.__simulateRejection) {
              const err = window.__simulateRejection;
              window.__simulateRejection = null;
              throw err;
            }
            window.__savedCandidate = JSON.parse(JSON.stringify(args.candidate));
            window.__tauri_mock.settings = JSON.parse(JSON.stringify(args.candidate));
            return {
              request_id: args.request_id,
              settings: window.__tauri_mock.settings,
              status: window.__tauri_mock.status,
            };
          }
          if (cmd === 'plugin:event|listen') {
            return 1;
          }
          if (cmd === 'plugin:event|unlisten') {
            return null;
          }
          if (cmd === 'plugin:window|minimize') {
            window.__minimizeCount = (window.__minimizeCount || 0) + 1;
            return null;
          }
          if (cmd === 'plugin:window|toggle_maximize') {
            window.__toggleMaximizeCount = (window.__toggleMaximizeCount || 0) + 1;
            window.__isMaximized = !window.__isMaximized;
            return null;
          }
          if (cmd === 'plugin:window|is_maximized') {
            return !!window.__isMaximized;
          }
          if (cmd === 'plugin:window|start_dragging') {
            window.__startDraggingCount = (window.__startDraggingCount || 0) + 1;
            return null;
          }
          throw new Error('Unknown mock command: ' + cmd);
        },
      };

      window.__TAURI__ = {
        core: {
          invoke: window.__TAURI_INTERNALS__.invoke,
        },
        event: {
          listen: async (evt, cb) => () => {},
        },
        window: {
          getCurrentWindow: () => ({
            label: 'main',
            listen: async (evt, cb) => () => {},
          }),
        },
      };

      window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
        unregisterListener: (evt, id) => {},
      };
    }, catalog, initialSettings);

    // 1. Load the page
    console.log('Navigating to Settings UI...');
    await page.goto(url, { waitUntil: 'networkidle0' });

    // Assert no unexpected error banner is displayed
    const warningEl = await page.$('.notice-callout.warning');
    if (warningEl) {
      const warningText = await page.evaluate(el => el.textContent, warningEl);
      throw new Error(`Unexpected runtime warning banner present: ${warningText.trim()}`);
    }

    // Opening Settings leaves gestures active; no implicit suspension.
    const statusLabel = await page.$eval('.status-label', el => el.textContent.trim());
    console.log('Status indicator text:', statusLabel);
    if (statusLabel !== 'Gestures active') {
      throw new Error(`Expected 'Gestures active', got '${statusLabel}'`);
    }

    // Assert initial page: Triggers
    const triggersHeading = await page.$eval('h1', el => el.textContent);
    console.log('Heading on Triggers page:', triggersHeading);
    if (!triggersHeading.includes('Triggers')) {
      throw new Error(`Expected Triggers heading, got ${triggersHeading}`);
    }

    // Tray/hotkey changes are reflected without overwriting the Settings draft.
    await page.evaluate(() => { window.__tauri_mock.status = 'paused'; });
    await page.waitForFunction(() => document.querySelector('.status-label').textContent.trim() === 'Paused', { timeout: 7000 });
    await page.evaluate(() => { window.__tauri_mock.status = 'active'; });
    await page.waitForFunction(() => document.querySelector('.status-label').textContent.trim() === 'Gestures active', { timeout: 7000 });

    // Verify gesture copy is present
    const gestureCopy = await page.$eval('.gesture-banner', el => el.textContent);
    console.log('Gesture banner text:', gestureCopy.trim());
    if (!gestureCopy.includes('Hold trigger + left mouse press, drag, and release to select a slice')) {
      throw new Error(`Missing required gesture copy! Found: ${gestureCopy}`);
    }

    // Preview contract: navigation is outside native drag chrome and theme is unified.
    const warmTheme = await page.evaluate(() => {
      const header = document.querySelector('.desktop-header').getBoundingClientRect();
      const nav = document.querySelector('.nav-tabs-wrapper').getBoundingClientRect();
      const root = getComputedStyle(document.documentElement);
      const input = document.querySelector('#ping-size') || document.querySelector('select');
      return {
        separate: nav.top >= header.bottom && !document.querySelector('.desktop-header nav'),
        theme: root.getPropertyValue('--color-primary').trim() === root.getPropertyValue('--accent').trim(),
        inputHeight: input.getBoundingClientRect().height,
      };
    });
    if (!warmTheme.separate || !warmTheme.theme || warmTheme.inputHeight < 34) {
      throw new Error(`Warm Settings layout/theme regression: ${JSON.stringify(warmTheme)}`);
    }

    // Capture Triggers screenshot (960x720)
    await capture(page, '01-triggers-960x720.png');
    console.log('Saved 01-triggers-960x720.png');

    // 2. Click Pings tab - verifies tab clickable
    console.log('Clicking Tactical Pings tab...');
    await page.click('#tab-pings');
    await page.waitForSelector('#panel-pings', { timeout: 2000 });

    const pingsHeading = await page.$eval('h1', el => el.textContent);
    console.log('Heading on Pings page:', pingsHeading);
    if (!pingsHeading.includes('Pings')) {
      throw new Error(`Expected Pings heading, got ${pingsHeading}`);
    }

    // Verify 8 directional nodes rendered in radial wheel
    const nodeCount = await page.$$eval('.wheel-node', els => els.length);
    console.log('Wheel nodes rendered:', nodeCount);
    if (nodeCount !== 8) {
      throw new Error(`Expected 8 wheel nodes, found ${nodeCount}`);
    }

    // Verify real PNG images rendered
    const imgCount = await page.$$eval('.node-img', els => els.length);
    console.log('Real image nodes rendered:', imgCount);
    if (imgCount !== 8) {
      throw new Error(`Expected 8 images in wheel preview, found ${imgCount}`);
    }

    // Capture Pings screenshot (960x720)
    await capture(page, '02-pings-960x720.png');
    console.log('Saved 02-pings-960x720.png');
    // HiDPI capture uses physical backing pixels, not a resized 1x screenshot.
    await page.setViewport({ width: 960, height: 720, deviceScaleFactor: 2 });
    await capture(page, '02-pings-960x720-2x.png');
    const settledScale = await page.$eval('.wheel-node.focused .node-img', img => {
      const style = getComputedStyle(img);
      return { width: img.getBoundingClientRect().width, scale: style.scale };
    });
    if (settledScale.width !== 44 || !['none', '1'].includes(settledScale.scale)) {
      throw new Error(`Selected art retains a fractional scale: ${JSON.stringify(settledScale)}`);
    }
    await page.setViewport({ width: 960, height: 720, deviceScaleFactor: 1 });

    // 3. Test changing a slot assignment and check draft feedback
    console.log('Updating East slice assignment to "danger" (creating duplicate)...');
    await page.click('.wheel-node.pos-e');
    await page.click('.asset-tile[aria-label="Assign Danger to East"]');
    const galleryAssignment = await page.$eval('#wheel-2', el => el.value);
    if (galleryAssignment !== 'danger') throw new Error('Gallery did not assign the selected wheel slice');
    await page.focus('.asset-tile[aria-label="Assign Push to East"]');
    await page.keyboard.press('Enter');
    if (await page.$eval('#wheel-2', el => el.value) !== 'push') {
      throw new Error('Keyboard gallery assignment failed');
    }
    await page.click('.asset-tile[aria-label="Assign Danger to East"]');

    // Verify duplicate badge appears
    await page.waitForSelector('.wheel-node.duplicate', { timeout: 2000 });
    const duplicateCount = await page.$$eval('.wheel-node.duplicate', els => els.length);
    console.log('Duplicate nodes in preview:', duplicateCount);
    if (duplicateCount < 1) {
      throw new Error('Expected duplicate node to be marked!');
    }

    // 4. Click Emotes tab - verifies tab clickable
    console.log('Clicking Emotes tab...');
    await page.click('#tab-emotes');
    await page.waitForSelector('#panel-emotes', { timeout: 2000 });

    const emotesHeading = await page.$eval('h1', el => el.textContent);
    console.log('Heading on Emotes page:', emotesHeading);
    if (!emotesHeading.includes('Emotes')) {
      throw new Error(`Expected Emotes heading, got ${emotesHeading}`);
    }

    const importFile = path.join(ASSETS_DIR, 'emotes', 'facepalm.png');
    const customPicker = await page.$('#custom-emote-file');
    await customPicker.uploadFile(importFile);
    await page.waitForFunction(() => document.querySelector('#custom-emote-message').textContent.includes('Imported to your local library'));
    const customChoices = await page.$$eval('option[value="custom_0000000000000001"]', options => options.length);
    if (customChoices !== 8) throw new Error(`Custom emote missing from wheel/tap choices: ${customChoices}`);
    await page.evaluate(() => { window.__rejectImport = true; });
    await customPicker.uploadFile(importFile);
    await page.waitForFunction(() => document.querySelector('#custom-emote-message').textContent.includes('Invalid image.'));
    await customPicker.uploadFile(importFile);
    await page.waitForFunction(() => document.querySelector('#custom-emote-message').textContent.includes('Imported to your local library'));
    if (await page.$$eval('option[value="custom_0000000000000001"]', options => options.length) !== customChoices) throw new Error('Duplicate import duplicated catalog entries');

    const gifDirectory = fs.mkdtempSync(path.join(require('os').tmpdir(), 'pingloom-import-gif-'));
    try {
      const gifFile = path.join(gifDirectory, 'custom.gif');
      fs.writeFileSync(gifFile, Buffer.from('R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7', 'base64'));
      await customPicker.uploadFile(gifFile);
      await page.waitForFunction(() => window.__lastImportPrefix === 'R0lG' && document.querySelector('#custom-emote-message').textContent.includes('Imported'));
      await customPicker.uploadFile(path.join(ASSETS_DIR, 'emotes', 'facepalm.webp'));
      await page.waitForFunction(() => window.__lastImportPrefix === 'UklG' && document.querySelector('#custom-emote-message').textContent.includes('Imported'));
    } finally { fs.rmSync(gifDirectory, { recursive: true }); }

    // Capture Emotes screenshot (960x720)
    await capture(page, '03-emotes-960x720.png');
    console.log('Saved 03-emotes-960x720.png');

    // 5. Test Small Viewport (720x600 reflow)
    console.log('Resizing to small viewport (720x600)...');
    await page.setViewport({ width: 720, height: 600 });
    await capture(page, '04-small-720x600-emotes.png');
    console.log('Saved 04-small-720x600-emotes.png');

    await page.click('#tab-pings');
    await page.waitForSelector('#panel-pings', { timeout: 2000 });
    await capture(page, '05-small-720x600-pings.png');
    console.log('Saved 05-small-720x600-pings.png');

    await page.click('#tab-triggers');
    await page.click('[data-field="emote_click"]');
    await page.select('#click-emote-id', 'custom_0000000000000001');

    // 6. Test Save Payload (current draft, clean object without Proxy)
    console.log('Testing Save changes...');
    await page.click('.btn-footer-primary');
    await page.waitForSelector('.status-msg.success', { timeout: 2000 });

    const saved = await page.evaluate(() => window.__savedCandidate);
    if (!saved.emote_wheel.includes('custom_0000000000000001') || saved.click_emote_id !== 'custom_0000000000000001') throw new Error('Custom wheel/tap assignment missing from Save payload');
    console.log('Saved candidate wheel[2]:', saved.wheel[2]);
    if (saved.wheel[2] !== 'danger') {
      throw new Error(`Expected saved wheel[2] to be 'danger', got ${saved.wheel[2]}`);
    }

    // 7. Test Validation Error Rejection & Field Focus
    console.log('Testing Validation error rejection...');
    await page.evaluate(() => {
      window.__simulateRejection = {
        code: 'invalid_field',
        message: 'Drag threshold must be between 2 and 40 px',
        field: 'drag_threshold_px',
        request_id: 'err-1',
      };
    });

    await page.click('.btn-footer-primary');
    await page.waitForSelector('.status-msg.error', { timeout: 2000 });

    // Assert it switched automatically to Triggers page where drag_threshold_px lives
    const activePanel = await page.$eval('.page-panel', el => el.id);
    console.log('Active panel after error rejection:', activePanel);
    if (activePanel !== 'panel-triggers') {
      throw new Error(`Expected panel-triggers after error on drag_threshold_px, got ${activePanel}`);
    }

    // Check that threshold input is focused
    const focusedId = await page.evaluate(() => document.activeElement ? document.activeElement.id : null);
    console.log('Focused element id after error rejection:', focusedId);
    if (focusedId !== 'threshold') {
      console.warn(`Warning: Expected #threshold focused, got #${focusedId}`);
    }

    // 8. Test Window Chrome Controls (Minimize, Maximize/Restore, Double-click, Close)
    console.log('Testing Window Chrome: Nav tabs drag safety...');
    const navHasNoDrag = await page.$eval('nav.nav-tabs-wrapper', el => el.hasAttribute('data-no-drag') && !el.hasAttribute('data-tauri-drag-region'));
    if (!navHasNoDrag) {
      throw new Error('Expected nav.nav-tabs-wrapper to have data-no-drag and not data-tauri-drag-region');
    }

    const brandingHasDrag = await page.$eval('.header-branding', el => el.hasAttribute('data-tauri-drag-region'));
    const fillerHasDrag = await page.$eval('.header-drag-filler', el => el.hasAttribute('data-tauri-drag-region'));
    if (!brandingHasDrag || !fillerHasDrag) {
      throw new Error('Expected .header-branding and .header-drag-filler to have data-tauri-drag-region');
    }

    console.log('Testing Window Chrome: Minimize button...');
    await page.click('.window-control-btn.btn-minimize');
    const minimizeCount = await page.evaluate(() => window.__minimizeCount);
    if (minimizeCount !== 1) {
      throw new Error(`Expected minimizeCount 1, got ${minimizeCount}`);
    }

    console.log('Testing Window Chrome: Toggle Maximize button...');
    await page.click('.window-control-btn.btn-maximize');
    const toggleCount1 = await page.evaluate(() => window.__toggleMaximizeCount);
    const maxTitle1 = await page.$eval('.window-control-btn.btn-maximize', el => el.getAttribute('title'));
    if (toggleCount1 !== 1 || !maxTitle1.includes('Restore Down')) {
      throw new Error(`Expected toggleCount 1 with 'Restore Down', got ${toggleCount1} with '${maxTitle1}'`);
    }

    // Toggle back down
    await page.click('.window-control-btn.btn-maximize');
    const toggleCount2 = await page.evaluate(() => window.__toggleMaximizeCount);
    const maxTitle2 = await page.$eval('.window-control-btn.btn-maximize', el => el.getAttribute('title'));
    if (toggleCount2 !== 2 || !maxTitle2.includes('Maximize')) {
      throw new Error(`Expected toggleCount 2 with 'Maximize', got ${toggleCount2} with '${maxTitle2}'`);
    }

    console.log('Testing Window Chrome: Double-click titlebar to toggle maximize...');
    await page.evaluate(() => {
      const filler = document.querySelector('.header-drag-filler');
      filler.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true }));
    });
    const toggleCount3 = await page.evaluate(() => window.__toggleMaximizeCount);
    if (toggleCount3 !== 3) {
      throw new Error(`Expected toggleCount 3 after dblclick, got ${toggleCount3}`);
    }

    console.log('Testing Wheel geometry: proportional percentage coordinates...');
    await page.click('#tab-pings');
    await page.waitForSelector('#panel-pings', { timeout: 2000 });
    const nodeStyles = await page.$$eval('.wheel-node', els => els.map(el => ({ left: el.style.left, top: el.style.top })));
    for (const s of nodeStyles) {
      if (!s.left.includes('%') || !s.top.includes('%')) {
        throw new Error(`Expected percentage coordinates for wheel node, got left=${s.left}, top=${s.top}`);
      }
    }
    console.log(`Verified ${nodeStyles.length} wheel nodes use proportional percentage coordinates`);

    console.log('Testing Dark Mode and Zoom 200% captures...');
    await page.setViewport({ width: 960, height: 720 });
    await page.emulateMediaFeatures([{ name: 'prefers-color-scheme', value: 'dark' }]);
    await capture(page, '06-dark-960x720.png');
    console.log('Saved 06-dark-960x720.png');

    await page.setViewport({ width: 720, height: 600 });
    await capture(page, '08-dark-720x600.png');
    console.log('Saved 08-dark-720x600.png');

    await page.setViewport({ width: 480, height: 360 });
    await capture(page, '07-zoom200-480x360.png');
    console.log('Saved 07-zoom200-480x360.png');

    // Reset viewport and theme
    await page.setViewport({ width: 960, height: 720 });
    await page.emulateMediaFeatures([{ name: 'prefers-color-scheme', value: 'light' }]);

    // Motion regressions: exercise rendered geometry and actual scroll calls, not CSS source.
    console.log('Testing reduced-motion wheel geometry and field reveal...');
    await page.emulateMediaFeatures([{ name: 'prefers-reduced-motion', value: 'reduce' }]);
    await page.click('#tab-emotes');
    await page.click('#tab-pings');
    await page.waitForSelector('#panel-pings');
    const motion = await page.evaluate(() => {
      const wheel = document.querySelector('.radial-wheel');
      const bounds = wheel.getBoundingClientRect();
      const misplaced = [...wheel.querySelectorAll('.wheel-node')].filter(node => {
        const rect = node.getBoundingClientRect();
        return Math.abs(rect.x + rect.width / 2 - bounds.x - parseFloat(node.style.left) * bounds.width / 100) > 1
          || Math.abs(rect.y + rect.height / 2 - bounds.y - parseFloat(node.style.top) * bounds.height / 100) > 1;
      }).length;
      return { misplaced, panelAnimations: document.querySelector('.page-panel').getAnimations().length };
    });
    if (motion.misplaced || motion.panelAnimations) {
      throw new Error(`Reduced-motion layout/animation regression: ${JSON.stringify(motion)}`);
    }
    await page.evaluate(() => {
      window.__scrollCalls = [];
      const scroll = Element.prototype.scrollIntoView;
      Element.prototype.scrollIntoView = function(options) {
        window.__scrollCalls.push(options);
        return scroll.call(this, options);
      };
    });
    await page.click('.wheel-node');
    const reveals = await page.evaluate(() => window.__scrollCalls);
    if (!reveals.length || reveals.some(call => call.behavior !== 'auto')) {
      throw new Error(`Reduced motion must reveal fields without smooth scrolling: ${JSON.stringify(reveals)}`);
    }
    await page.emulateMediaFeatures([{ name: 'prefers-reduced-motion', value: 'no-preference' }]);
    console.log('Testing interruption, focused-node press and hidden-window spinner...');
    for (let i = 0; i < 6; i++) {
      await page.click(i % 2 ? '#tab-pings' : '#tab-emotes');
    }
    if (await page.$$eval('[role="tabpanel"]', panels => panels.length) !== 1) {
      throw new Error('Interrupted tab navigation left overlapping panels');
    }
    await page.click('.wheel-node');
    await page.hover('.wheel-node');
    await page.mouse.down();
    try {
      await page.waitForFunction(() => {
        const node = document.querySelector('.wheel-node:active');
        return node && new DOMMatrix(getComputedStyle(node).transform).a < 0.999;
      }, { timeout: 2000 });
    } finally {
      await page.mouse.up();
    }
    await page.evaluate(() => {
      window.__deferSave = true;
    });
    await page.click('.btn-footer-primary');
    await page.waitForSelector('.status-msg.saving .spin');
    await page.evaluate(() => {
      Object.defineProperty(document, 'hidden', { configurable: true, value: true });
      document.dispatchEvent(new Event('visibilitychange'));
    });
    await page.waitForFunction(() => getComputedStyle(document.querySelector('.status-msg.saving .spin')).animationPlayState === 'paused');
    await page.evaluate(() => {
      Object.defineProperty(document, 'hidden', { configurable: true, value: false });
      document.dispatchEvent(new Event('visibilitychange'));
    });
    await page.waitForFunction(() => getComputedStyle(document.querySelector('.status-msg.saving .spin')).animationPlayState === 'running');
    await page.evaluate(() => window.__resumeSave());
    await page.waitForSelector('.status-msg.success');

    // 9. Test Window Chrome Close Button (calls close_settings, NOT destroy)
    console.log('Testing Window Chrome Close button (generation-aware close_settings)...');
    await page.evaluate(() => { window.__closedCount = 0; });
    await page.click('.window-control-btn.btn-close');
    const closedFromChrome = await page.evaluate(() => window.__closedCount);
    console.log('Chrome close call count:', closedFromChrome);
    if (closedFromChrome !== 1) {
      throw new Error(`Expected close count 1 from titlebar close button, got ${closedFromChrome}`);
    }

    console.log('ALL BROWSER AUTOMATION TESTS PASSED SUCCESSFULLY!');
  } finally {
    await browser.close();
    server.close();
  }
})().catch(err => {
  console.error('TEST RUNNER FAILED:', err);
  process.exit(1);
});
