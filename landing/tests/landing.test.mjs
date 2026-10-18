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
  const response = await worker.fetch(new Request('https://test/missing'), { ASSETS: { fetch: () => new Response('Not found', { status: 404 }) } });
  assert.equal(response.status, 404);
});
