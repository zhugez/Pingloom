// Optional browser check: start `npm run dev`, then provide Playwright Core and Chromium paths.
import assert from 'node:assert/strict';
import { mkdir } from 'node:fs/promises';
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright-core');
const browser = await chromium.launch({ executablePath: process.env.CHROME_PATH, headless: true, args: ['--no-sandbox'] });
const base = process.env.LANDING_URL || 'http://localhost:8787';
const captures = new URL('../../.impeccable/review/landing/', import.meta.url);
await mkdir(captures, { recursive: true });
try {
  for (const width of [1440, 390, 320]) {
    const page = await browser.newPage({ viewport: { width, height: 1000 }, reducedMotion: 'reduce' });
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    page.on('response', response => { if (response.status() >= 400) errors.push(`${response.status()} ${response.url()}`); });
    await page.goto(base);
    await page.locator('.wheel-node').first().waitFor();
    assert.equal(await page.locator('.wheel-node').count(), 8);
    assert.equal(await page.locator('select').count(), 0);
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false, `${width}px overflow`);
    const catalog = await (await page.request.get(`${base}/catalog.json`)).json();
    for (const mode of ['pings', 'emotes']) {
      await page.locator(`[data-mode="${mode}"]`).click();
      for (let index = 0; index < 8; index++) {
        const node = page.locator('.wheel-node').nth(index);
        assert.equal(await node.locator('img').getAttribute('src'), catalog[mode][index].image);
        await node.focus();
        await page.keyboard.press('Enter');
        assert.equal(await page.locator('#selected-name').textContent(), catalog[mode][index].name);
        // Reduced-motion uses the original still asset, not an animated WebP.
        assert.equal(await page.locator('#selected-art').getAttribute('src'), catalog[mode][index].image);
      }
    }
    await page.locator('[data-mode="pings"]').click();
    await page.locator('#wheel').scrollIntoViewIfNeeded();
    const rect = await page.locator('#wheel').boundingBox();
    const cx = rect.x + rect.width / 2, cy = rect.y + rect.height / 2;
    // Drag to each direction at the actual rendered scale.
    for (let slot = 0; slot < 8; slot++) {
      const angle = slot * Math.PI / 4;
      await page.mouse.move(cx, cy);
      await page.mouse.down();
      await page.mouse.move(cx + Math.sin(angle) * rect.width * .34, cy - Math.cos(angle) * rect.height * .34);
      await page.mouse.up();
      assert.equal(await page.locator('#selected-name').textContent(), catalog.pings[slot].name);
    }
    await page.mouse.move(cx, cy);
    await page.mouse.down();
    await page.mouse.move(cx, cy - rect.height * .34);
    await page.mouse.move(cx, cy);
    await page.mouse.up();
    assert.equal(await page.locator('#announcement').textContent(), 'Gesture cancelled.');
    await page.mouse.move(cx, cy);
    await page.mouse.down();
    await page.mouse.move(cx, cy - rect.height * .34);
    await page.locator('#wheel').dispatchEvent('pointercancel', { pointerId: 1 });
    await page.mouse.up();
    assert.equal(await page.locator('.wedge.selected').count(), 0);
    assert.equal(await page.locator('#announcement').textContent(), 'Gesture cancelled.');
    await page.locator('#sound').click();
    assert.equal(await page.locator('#sound').getAttribute('aria-pressed'), 'true');
    const [soundResponse] = await Promise.all([
      page.waitForResponse(response => response.url().endsWith(catalog.pings[7].sound)),
      page.locator('#replay').click(),
    ]);
    assert.ok(soundResponse.ok());
    await page.locator('#sound').click();
    // Real emote animation in normal motion; a still asset in reduced motion.
    await page.emulateMedia({ reducedMotion: 'no-preference' });
    await page.locator('[data-mode="emotes"]').click();
    await page.locator('.wheel-node').first().focus();
    await page.keyboard.press('Enter');
    assert.equal(await page.locator('#selected-art').getAttribute('src'), catalog.emotes[0].effect);
    await page.locator('#selected-art').evaluate(image => image.decode());
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.locator('[data-mode="pings"]').click();
    await page.locator('#app').scrollIntoViewIfNeeded();
    await page.evaluate(async () => { await document.fonts.ready; await Promise.all([...document.images].map(image => image.decode())); });
    assert.deepEqual(errors, []);
    await page.screenshot({ path: new URL(`page-${width}.png`, captures).pathname, fullPage: true });
    console.log(`${width}px: artwork, modes, keyboard, drag/cancel, motion and layout OK`);
    await page.close();
  }
} finally {
  await browser.close();
}
