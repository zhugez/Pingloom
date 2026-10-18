import { readFile, copyFile, mkdir, rm, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

// Use the desktop catalogue as the source of truth, not a second hand-maintained list.
const root = new URL('../', import.meta.url);
const output = new URL('./public/', import.meta.url);
const model = await readFile(new URL('src/model.rs', root), 'utf8');
function block(name) {
  const value = model.match(new RegExp(`pub const ${name}[^=]*=\\s*&?\\[([\\s\\S]*?)\\];`))?.[1];
  if (!value) throw new Error(`Missing desktop catalogue: ${name}`);
  return value;
}
const pingAssets = [...block('PING_ASSETS').matchAll(/PingAsset\s*\{([\s\S]*?)\}/g)].map(([, item]) => {
  const field = name => item.match(new RegExp(`${name}:\\s*"([^"]+)"`))?.[1];
  return {
    id: item.match(/id:\s*PingId::(\w+)/)?.[1], name: field('name'), color: field('color'),
    image: `/assets/textures/${field('icon')}.png`,
    effect: `/assets/textures/${item.match(/art:\s*&\["([^"]+)"/)?.[1]}.png`,
    sound: `/assets/sounds/${field('sound')}.ogg`,
  };
});
const pings = [...block('DEFAULT_PING_WHEEL').matchAll(/PingId::(\w+)/g)].map(([, id]) => pingAssets.find(ping => ping.id === id));
const emotes = [...block('DEFAULT_EMOTE_WHEEL').matchAll(/"([^"]+)"/g)].map(([, id]) => ({
  id, name: id.split('_').map(word => word[0].toUpperCase() + word.slice(1)).join(' '),
  image: `/assets/emotes/${id}.png`, effect: `/assets/emotes/${id}.webp`, sound: `/assets/emotes/${id}.ogg`,
}));
if (pings.length !== 8 || emotes.length !== 8 || pings.some(ping => !ping)) throw new Error('Expected two eight-slice desktop Wheels');
await rm(output, { recursive: true, force: true });
await mkdir(output, { recursive: true });
async function copy(source, destination) {
  const target = new URL(destination, output);
  await mkdir(new URL('.', target), { recursive: true });
  await copyFile(new URL(source, root), target);
}
for (const item of [...pings, ...emotes]) {
  for (const key of ['image', 'effect', 'sound']) await copy(item[key].slice(1), item[key].slice(1));
}
for (const file of ['pingloom-sapphire-dark-preview.png', 'pingloom-native-wheel-v7.png']) await copy(`docs/media/${file}`, `images/${file}`);
await copy('docs/media/brand/pingloom-mark-mono.svg', 'images/mark.svg');
await copy('THIRD_PARTY_NOTICES.txt', 'THIRD_PARTY_NOTICES.txt');
await writeFile(new URL('googled7ecf9fd7d293ff6.html', output), 'google-site-verification: googled7ecf9fd7d293ff6.html\n');
for (const file of ['bricolage-grotesque-latin-wght-normal.woff2', 'plus-jakarta-sans-latin-wght-normal.woff2', 'OFL.txt', 'OFL-bricolage-grotesque.txt']) await copy(`ui/src/fonts/${file}`, `fonts/${file}`);
for (const file of ['style.css', 'demo.js']) await copy(`landing/src/${file}`, file);
await writeFile(new URL('catalog.json', output), JSON.stringify({ pings, emotes }));
console.log(`Prepared desktop assets in ${fileURLToPath(output)}`);
