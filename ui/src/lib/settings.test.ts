import test from 'node:test';
import assert from 'node:assert/strict';
import { cloneSettings, DEFAULT_EMOTE_WHEEL, DEFAULT_PING_WHEEL, DIRECTIONS, DIRECTION_LABELS, duplicateSlots, formatAssetName, pageForField, sectorPath, triggerLabel, type Settings } from './settings';

test('duplicate wheel slots mark every occurrence after the first', () => {
  assert.deepEqual([...duplicateSlots(['a', 'b', 'a', 'c', 'b'])], [2, 4]);
});
test('asset names are readable while preserving their persisted ids', () => {
  assert.equal(formatAssetName('gg_heart'), 'Gg Heart');
});
test('trigger display handles Off and custom keys', () => {
  assert.equal(triggerLabel(null), 'Off');
  assert.equal(triggerLabel({ key: { vk: 81 } }), 'Key 81');
});
test('canonical error fields route to the page containing their control', () => {
  assert.equal(pageForField('click_emote_id'), 'emotes');
  assert.equal(pageForField('emote_wheel.3'), 'emotes');
  assert.equal(pageForField('wheel.3'), 'pings');
  assert.equal(pageForField('toggle_hotkey.vk'), 'triggers');
});
test('draft clone handles reactive settings and is isolated from canonical state', () => {
  const original = new Proxy({
    toggle_hotkey: { mods: 3, vk: 80 },
    trigger: { key: { vk: 81 } },
    emote_trigger: null,
    wheel: ['a', 'b'],
    emote_wheel: ['x', 'y'],
  } as unknown as Settings, {});
  const draft = cloneSettings(original);
  draft.toggle_hotkey.vk = 65;
  (draft.trigger as { key: { vk: number } }).key.vk = 66;
  draft.wheel[0] = 'changed';
  draft.emote_wheel[0] = 'changed';
  assert.equal(original.toggle_hotkey.vk, 80);
  assert.equal((original.trigger as { key: { vk: number } }).key.vk, 81);
  assert.equal(original.wheel[0], 'a');
  assert.equal(original.emote_wheel[0], 'x');
});

test('direction labels cover all 8 compass directions', () => {
  assert.equal(DIRECTIONS.length, 8);
  for (const dir of DIRECTIONS) {
    assert.ok(DIRECTION_LABELS[dir], `Missing label for ${dir}`);
  }
});

test('default wheels define all 8 slices', () => {
  assert.equal(DEFAULT_PING_WHEEL.length, 8);
  assert.equal(DEFAULT_EMOTE_WHEEL.length, 8);
});

test('sectorPath generates valid annular wedge SVG path data', () => {
  const d = sectorPath(140, 140, 50, 134, -Math.PI / 8, Math.PI / 8);
  assert.ok(d.startsWith('M '));
  assert.ok(d.includes(' A 134 134 '));
  assert.ok(d.includes(' A 50 50 '));
  assert.ok(d.endsWith(' Z'));
});
