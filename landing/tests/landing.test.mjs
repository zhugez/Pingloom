import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import worker from '../src/index.js';

const publicDir = new URL('../public/', import.meta.url);
const repo = new URL('../../', import.meta.url);
const catalog = JSON.parse(await readFile(new URL('catalog.json', publicDir)));

test('both Wheels use the desktop defaults and byte-identical artwork, animation and audio', async () => {
  const model = await readFile(new URL('src/model.rs', repo), 'utf8');
  const defaults = name => model.match(new RegExp(`pub const ${name}[^=]*=\\s*&?\\[([\\s\\S]*?)\\];`))[1];
  assert.deepEqual(catalog.pings.map(item => item.id), [...defaults('DEFAULT_PING_WHEEL').matchAll(/PingId::(\w+)/g)].map(match => match[1]));
  assert.deepEqual(catalog.emotes.map(item => item.id), [...defaults('DEFAULT_EMOTE_WHEEL').matchAll(/"([^"]+)"/g)].map(match => match[1]));
  assert.equal(catalog.pings.length, 8);
  assert.equal(catalog.emotes.length, 8);
  for (const item of [...catalog.pings, ...catalog.emotes]) {
    for (const key of ['image', 'effect', 'sound']) {
      const path = item[key].slice(1);
      assert.deepEqual(await readFile(new URL(path, publicDir)), await readFile(new URL(path, repo)));
    }
  }
  for (const file of ['pingloom-native-wheel-v7.png', 'pingloom-sapphire-dark-preview.png']) {
    assert.deepEqual(await readFile(new URL(`images/${file}`, publicDir)), await readFile(new URL(`docs/media/${file}`, repo)));
  }
});

test('page has one fixed theme, local previews, and no fabricated metrics', async () => {
  const page = await (await worker.fetch(new Request('https://test/'), {})).text();
  assert.match(page, /\/images\/pingloom-native-wheel-v7.png/);
  assert.match(page, /\/images\/pingloom-sapphire-dark-preview.png/);
  assert.doesNotMatch(page, /themeSelect|data-theme|0\.02ms|240 Hz|fonts.googleapis/);
  const demo = await readFile(new URL('../src/demo.js', import.meta.url), 'utf8');
  assert.doesNotMatch(demo, /localStorage|AudioContext/);
  for (const [, path] of page.matchAll(/(?:src|href)="(\/[^"#]*)"/g)) {
    if (path === '/' || path.startsWith('/download/')) continue;
    await readFile(new URL(path.slice(1), publicDir));
  }
});

test('download redirects and status remain available; unknown paths go to static assets', async () => {
  for (const platform of ['windows', 'windows-portable', 'macos']) {
    const response = await worker.fetch(new Request(`https://test/download/${platform}`), {});
    assert.equal(response.status, 302);
    assert.match(response.headers.get('location'), /^https:\/\/github.com\/zhugez\/pingloom\/releases\/download\/v0\.1\.0-opt\.2\//);
  }
  const status = await (await worker.fetch(new Request('https://test/api/status'), {})).json();
  assert.equal(status.project, 'Pingloom');
  assert.equal(status.version, '0.1.0');
  assert.equal(status.release_tag, 'v0.1.0-opt.2');
  assert.equal(status.deployment_url, 'https://pingloom.zhugez.workers.dev');
  assert.match(await readFile(new URL('../wrangler.toml', import.meta.url), 'utf8'), /^name = "pingloom"$/m);
  const response = await worker.fetch(new Request('https://test/missing'), { ASSETS: { fetch: () => new Response('Not found', { status: 404 }) } });
  assert.equal(response.status, 404);
});

test('displayed version stays v0.1.0 while all distribution links select the approved build', async () => {
  const config = await readFile(new URL('../wrangler.toml', import.meta.url), 'utf8');
  const env = {
    PINGLOOM_VERSION: config.match(/^PINGLOOM_VERSION = "([^"]+)"$/m)?.[1],
    PINGLOOM_RELEASE_TAG: config.match(/^PINGLOOM_RELEASE_TAG = "([^"]+)"$/m)?.[1],
  };
  assert.equal(env.PINGLOOM_VERSION, '0.1.0');
  assert.equal(env.PINGLOOM_RELEASE_TAG, 'v0.1.0-opt.2');
  const readme = await readFile(new URL('../../README.md', import.meta.url), 'utf8');
  assert.ok(!readme.includes('/releases/download/v0.1.0/'));
  const notes = 'https://github.com/zhugez/pingloom/releases/tag/v0.1.0-opt.2';
  assert.ok(readme.includes(`](${notes})`));
  for (const bindings of [{}, env]) {
    const html = await (await worker.fetch(new Request('https://test/'), bindings)).text();
    assert.ok(html.includes('Pingloom v0.1.0 · Prerelease'));
    assert.ok(!html.replace(/<[^>]+>/g, ' ').includes('0.1.0-opt'));
    assert.ok(html.includes(`href="${notes}">Release notes</a>`));
    const status = await (await worker.fetch(new Request('https://test/api/status'), bindings)).json();
    assert.equal(status.version, '0.1.0');
    assert.equal(status.release_tag, 'v0.1.0-opt.2');
    for (const [platform, field, filename] of [
      ['windows', 'windows_installer', 'Pingloom_0.1.0_x64-setup.exe'],
      ['windows-portable', 'windows_portable', 'pingloom.exe'],
      ['macos', 'macos_universal', 'pingloom-macos-universal.zip'],
    ]) {
      const expected = `https://github.com/zhugez/pingloom/releases/download/v0.1.0-opt.2/${filename}`;
      const redirect = await worker.fetch(new Request(`https://test/download/${platform}`), bindings);
      assert.equal(redirect.status, 302);
      assert.equal(redirect.headers.get('location'), expected);
      assert.equal(status.downloads[field], expected);
      assert.ok(readme.includes(`](${expected})`));
    }
  }
  const override = { PINGLOOM_VERSION: '0.1.0', PINGLOOM_RELEASE_TAG: 'v0.1.0-test' };
  const status = await (await worker.fetch(new Request('https://test/api/status'), override)).json();
  assert.equal(status.version, '0.1.0');
  assert.equal(status.release_tag, override.PINGLOOM_RELEASE_TAG);
  assert.ok(status.downloads.windows_installer.includes('/v0.1.0-test/'));
});

test('seo metadata, canonical, OpenGraph, Twitter and valid JSON-LD are served truthfully', async () => {
  const html = await (await worker.fetch(new Request('https://test/'), {})).text();
  assert.ok(html.includes('<link rel="canonical" href="https://pingloom.zhugez.workers.dev/">'));
  assert.ok(html.includes('<meta property="og:type" content="website">'));
  assert.ok(html.includes('<meta property="og:url" content="https://pingloom.zhugez.workers.dev/">'));
  assert.ok(html.includes('<meta name="google-site-verification" content="googled7ecf9fd7d293ff6">'));
  assert.ok(html.includes('<link rel="preload" href="/fonts/plus-jakarta-sans-latin-wght-normal.woff2" as="font" type="font/woff2" crossorigin>'));
  assert.ok(html.includes('<link rel="preload" href="/fonts/bricolage-grotesque-latin-wght-normal.woff2" as="font" type="font/woff2" crossorigin>'));
  assert.ok(html.includes('<link rel="preload" href="/images/pingloom-native-wheel-v7-640.webp" imagesrcset="/images/pingloom-native-wheel-v7-640.webp 640w, /images/pingloom-native-wheel-v7.webp 1004w" imagesizes="(max-width: 700px) calc(100vw - 36px), 630px" as="image" type="image/webp" fetchpriority="high">'));
  assert.ok(html.includes('<link rel="preload" href="/catalog.json" as="fetch" crossorigin>'));
  assert.ok(html.includes('<h1 id="hero-title"><span class="sr-only">Pingloom — </span>Eight directions.<br><span>One flick.</span></h1>'));
  const ogImage = 'https://pingloom.zhugez.workers.dev/images/pingloom-native-wheel-v7.png';
  assert.ok(html.includes(`<meta property="og:image" content="${ogImage}">`));
  assert.ok(html.includes('<meta property="og:image:width" content="1004">'));
  assert.ok(html.includes('<meta property="og:image:height" content="502">'));
  assert.ok(html.includes('<meta property="og:image:alt"'));
  assert.ok(html.includes('<meta name="twitter:card" content="summary_large_image">'));
  assert.ok(html.includes(`<meta name="twitter:image" content="${ogImage}">`));
  assert.ok(await readFile(new URL('images/pingloom-native-wheel-v7.png', publicDir)));

  const jsonLdMatch = html.match(/<script type="application\/ld\+json">([\s\S]*?)<\/script>/);
  assert.ok(jsonLdMatch);
  const schema = JSON.parse(jsonLdMatch[1]);
  assert.equal(schema['@context'], 'https://schema.org');
  assert.equal(schema['@type'], 'SoftwareApplication');
  assert.equal(schema.name, 'Pingloom');
  assert.equal(schema.softwareVersion, '0.1.0');
  assert.ok(schema.operatingSystem.includes('Windows 11') && schema.operatingSystem.includes('macOS'));
  assert.equal(schema.offers?.price, '0');
  assert.equal(schema.offers?.priceCurrency, 'USD');
  assert.equal(schema.url, 'https://pingloom.zhugez.workers.dev/');
  assert.equal(schema.sameAs, 'https://github.com/zhugez/pingloom');
  assert.equal(schema.license, 'https://opensource.org/licenses/MIT');
  assert.equal(schema.downloadUrl, 'https://pingloom.zhugez.workers.dev/#download');
  assert.equal(schema.author?.name, 'zhugez');
  assert.ok(Array.isArray(schema.featureList) && schema.featureList.length >= 4);
  assert.equal(schema.screenshot, ogImage);
  assert.equal(schema.aggregateRating, undefined);
  assert.equal(schema.review, undefined);
  assert.equal(schema.certification, undefined);
});

test('robots.txt and sitemap.xml serve valid crawl directives before asset fallback', async () => {
  const robotsRes = await worker.fetch(new Request('https://test/robots.txt'), {});
  assert.equal(robotsRes.status, 200);
  assert.ok(robotsRes.headers.get('content-type')?.includes('text/plain'));
  assert.ok(robotsRes.headers.get('cache-control')?.includes('max-age'));
  const robots = await robotsRes.text();
  assert.ok(robots.includes('User-agent: *'));
  assert.ok(robots.includes('Allow: /'));
  assert.ok(robots.includes('Sitemap: https://pingloom.zhugez.workers.dev/sitemap.xml'));

  const sitemapRes = await worker.fetch(new Request('https://test/sitemap.xml'), {});
  assert.equal(sitemapRes.status, 200);
  assert.ok(sitemapRes.headers.get('content-type')?.includes('xml'));
  assert.ok(sitemapRes.headers.get('cache-control')?.includes('max-age'));
  const sitemap = await sitemapRes.text();
  assert.ok(sitemap.includes('<loc>https://pingloom.zhugez.workers.dev/</loc>'));
  assert.equal([...sitemap.matchAll(/<loc>/g)].length, 1);
  assert.ok(!sitemap.includes('#'));
  assert.ok(!sitemap.includes('/download/'));
  assert.ok(!sitemap.includes('<lastmod>'));
});

test('desktop usage instructions, platform signing disclosures and checksum links are truthful and aligned', async () => {
  const html = await (await worker.fetch(new Request('https://test/'), {})).text();
  assert.ok(html.includes('Hold your keyboard trigger (default Alt for pings, Ctrl for emotes)'));
  assert.ok(html.includes('Keep the trigger held, press the left mouse button and drag toward a slice.'));
  assert.ok(html.includes('Release the left mouse button outside the centre to send.'));
  assert.ok(html.includes('Release inside the central cancellation zone, press Escape, or right-click to cancel'));
  assert.ok(html.includes('href="https://github.com/zhugez/pingloom#gestures--controls"'));
  assert.ok(html.includes('href="https://github.com/zhugez/pingloom/blob/main/docs/PLATFORMS.md"'));

  assert.ok(html.includes('SmartScreen'));
  assert.ok(html.includes('Unsigned prerelease'));
  assert.ok(html.includes('Ad-hoc signed, unnotarized'));
  assert.ok(html.includes('Gatekeeper'));
  assert.ok(html.includes('Accessibility'));
  assert.ok(html.includes('Input Monitoring'));
  assert.ok(!html.toLowerCase().includes('disable gatekeeper'));
  assert.ok(!html.toLowerCase().includes('disable antivirus'));

  const winChecksum = 'https://github.com/zhugez/pingloom/releases/download/v0.1.0-opt.2/BUILD_INFO-windows.txt';
  const macChecksum = 'https://github.com/zhugez/pingloom/releases/download/v0.1.0-opt.2/BUILD_INFO-macos.txt';
  assert.ok(html.includes(`href="${winChecksum}"`));
  assert.ok(html.includes(`href="${macChecksum}"`));
  assert.ok(!html.includes('SHA256SUMS.txt'));

  const visibleText = html.replace(/<[^>]+>/g, ' ');
  assert.ok(!visibleText.includes('0.1.0-opt'));
  assert.ok(visibleText.includes('v0.1.0'));

  const override = { PINGLOOM_VERSION: '0.1.0', PINGLOOM_RELEASE_TAG: 'v0.1.0-test' };
  const overrideHtml = await (await worker.fetch(new Request('https://test/'), override)).text();
  assert.ok(overrideHtml.includes('https://github.com/zhugez/pingloom/releases/download/v0.1.0-test/BUILD_INFO-windows.txt'));
  assert.ok(overrideHtml.includes('https://github.com/zhugez/pingloom/releases/download/v0.1.0-test/BUILD_INFO-macos.txt'));
});

test('google search console verification and llms.txt endpoints are served correctly', async () => {
  const gscRes = await worker.fetch(new Request('https://test/googled7ecf9fd7d293ff6.html'), {});
  assert.equal(gscRes.status, 200);
  assert.ok(gscRes.headers.get('content-type')?.includes('text/html'));
  assert.ok(gscRes.headers.get('cache-control')?.includes('max-age'));
  assert.equal(await gscRes.text(), 'google-site-verification: googled7ecf9fd7d293ff6.html\n');

  const llmsRes = await worker.fetch(new Request('https://test/llms.txt'), {});
  assert.equal(llmsRes.status, 200);
  assert.ok(llmsRes.headers.get('content-type')?.includes('text/markdown'));
  assert.ok(llmsRes.headers.get('cache-control')?.includes('max-age'));
  const llms = await llmsRes.text();
  assert.ok(llms.startsWith('# Pingloom'));
  assert.ok(llms.includes('League of Legends'));
  assert.ok(llms.includes('Windows 11 x64'));
  assert.ok(llms.includes('macOS (Apple Silicon & Intel)'));
  assert.ok(llms.includes('https://github.com/zhugez/pingloom'));
});
