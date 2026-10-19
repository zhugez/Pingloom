const wheel = document.querySelector('#wheel');
const wedges = document.querySelector('#wedges');
const nodes = document.querySelector('#nodes');
const art = document.querySelector('#selected-art');
const stage = document.querySelector('#effect-stage');
const announcement = document.querySelector('#announcement');
const replay = document.querySelector('#replay');
const soundButton = document.querySelector('#sound');
const directions = ['N · North', 'NE · North-east', 'E · East', 'SE · South-east', 'S · South', 'SW · South-west', 'W · West', 'NW · North-west'];
const reducedMotion = matchMedia('(prefers-reduced-motion: reduce)');
const audio = new Audio();
audio.volume = .35;
let sound = false;
let catalog;
let mode = 'pings';
let selected = 0;
let pointer = null;

function point(radius, angle) {
  return [230 + radius * Math.sin(angle), 230 - radius * Math.cos(angle)];
}
function sector(index) {
  const start = index * Math.PI / 4 - Math.PI / 8 + .025;
  const end = start + Math.PI / 4 - .05;
  return `M ${point(94, start)} L ${point(212, start)} A 212 212 0 0 1 ${point(212, end)} L ${point(94, end)} A 94 94 0 0 0 ${point(94, start)} Z`;
}
function highlight(slot) {
  [...wedges.children].forEach((wedge, index) => wedge.classList.toggle('selected', slot === index));
  [...nodes.children].forEach((node, index) => node.setAttribute('aria-pressed', String(slot === index)));
}
function inspect(slot) {
  selected = slot;
  const item = catalog[mode][slot];
  art.src = item.image;
  art.alt = item.name;
  art.hidden = false;
  document.querySelector('#selected-name').textContent = item.name;
  document.querySelector('#direction').textContent = directions[slot];
}
function preview(slot) {
  if (slot === null) return;
  audio.pause();
  inspect(slot);
  highlight(slot);
  const item = catalog[mode][slot];
  art.src = reducedMotion.matches ? item.image : item.effect;
  stage.classList.remove('playing');
  void stage.offsetWidth;
  stage.classList.add('playing');
  announcement.textContent = `${item.name} preview.`;
  if (sound) {
    audio.src = item.sound;
    audio.play().catch(() => { announcement.textContent = `${item.name} preview. Audio could not play; check browser sound permissions.`; });
  }
}
function render() {
  wedges.replaceChildren();
  nodes.replaceChildren();
  catalog[mode].forEach((item, index) => {
    const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
    path.setAttribute('d', sector(index));
    path.setAttribute('class', 'wedge');
    wedges.append(path);
    const node = document.createElement('button');
    node.type = 'button';
    node.className = 'wheel-node';
    node.setAttribute('aria-label', `Preview ${item.name}, ${directions[index]}`);
    const [x, y] = point(156, index * Math.PI / 4);
    node.style.left = `${x / 460 * 100}%`;
    node.style.top = `${y / 460 * 100}%`;
    const image = document.createElement('img');
    image.src = item.image;
    image.alt = '';
    image.draggable = false;
    image.loading = 'lazy';
    image.decoding = 'async';
    image.width = 58;
    image.height = 58;
    node.append(image);
    // Native button activation covers keyboard and assistive technology; pointer release is handled below.
    node.addEventListener('click', event => { if (event.detail === 0) preview(index); });
    nodes.append(node);
  });
  inspect(0);
  highlight(0);
}
function slotAt(event) {
  const rect = wheel.getBoundingClientRect();
  const x = (event.clientX - rect.left) * 460 / rect.width - 230;
  const y = (event.clientY - rect.top) * 460 / rect.height - 230;
  if (Math.hypot(x, y) < 86) return null;
  return Math.floor((Math.atan2(y, x) * 180 / Math.PI + 112.5 + 360) % 360 / 45);
}
function cancel() {
  const id = pointer;
  pointer = null;
  wheel.classList.remove('dragging');
  if (id !== null && wheel.hasPointerCapture(id)) wheel.releasePointerCapture(id);
  highlight(null);
}
wheel.addEventListener('pointerdown', event => {
  if (!catalog || pointer !== null || !event.isPrimary || event.button !== 0) return;
  pointer = event.pointerId;
  wheel.setPointerCapture(pointer);
  wheel.classList.add('dragging');
  highlight(slotAt(event));
});
wheel.addEventListener('pointermove', event => {
  if (pointer === event.pointerId) highlight(slotAt(event));
});
wheel.addEventListener('pointerup', event => {
  if (pointer !== event.pointerId) return;
  const slot = slotAt(event);
  cancel();
  if (slot === null) announcement.textContent = 'Gesture cancelled.';
  else preview(slot);
});
wheel.addEventListener('pointercancel', cancel);
wheel.addEventListener('lostpointercapture', () => { if (pointer !== null) cancel(); });
window.addEventListener('blur', cancel);
wheel.addEventListener('keydown', event => {
  if (event.key === 'Escape') { cancel(); announcement.textContent = 'Gesture cancelled.'; }
});
replay.addEventListener('click', () => preview(selected));
soundButton.addEventListener('click', () => {
  sound = !sound;
  soundButton.setAttribute('aria-pressed', String(sound));
  soundButton.textContent = sound ? 'Sound on' : 'Sound off';
  if (!sound) audio.pause();
});
for (const button of document.querySelectorAll('[data-mode]')) {
  button.addEventListener('click', () => {
    if (!catalog) return;
    cancel();
    audio.pause();
    mode = button.dataset.mode;
    for (const other of document.querySelectorAll('[data-mode]')) other.setAttribute('aria-pressed', String(other === button));
    render();
    announcement.textContent = `${mode === 'pings' ? 'Tactical Pings' : 'Emotes'} Wheel selected.`;
  });
}
const embeddedCatalog = document.getElementById('catalog-data');
if (embeddedCatalog) {
  try {
    catalog = JSON.parse(embeddedCatalog.textContent);
    render();
    replay.disabled = false;
  } catch {
    document.querySelector('#demo-error').hidden = false;
  }
} else {
  try {
    const response = await fetch('/catalog.json');
    if (!response.ok) throw new Error('Catalogue unavailable');
    catalog = await response.json();
    render();
    replay.disabled = false;
  } catch {
    document.querySelector('#demo-error').hidden = false;
  }
}
