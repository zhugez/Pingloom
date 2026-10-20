export type Trigger = 'alt' | 'ctrl' | 'shift' | 'win' | 'caps_lock' | 'mouse4' | 'mouse5' | { key: { vk: number } };
export interface Settings {
  version: 1;
  enabled_on_start: boolean;
  trigger: Trigger;
  emote_trigger: Trigger | null;
  toggle_hotkey: { mods: number; vk: number };
  drag_threshold_px: number;
  click_ping: boolean;
  click_ping_id: string;
  game_check: boolean;
  wheel: string[];
  wheel_size_percent: number;
  ping_size_px: number;
  ping_duration_s: number;
  volume: number;
  muted: boolean;
  tick_sound: boolean;
  emote_click: boolean;
  click_emote_id: string;
  emote_wheel: string[];
  emote_size_px: number;
  emote_sound: boolean;
}
export interface CatalogEntry { id: string; name: string; preview_url: string }
export interface BootData {
  session_id: string;
  platform?: 'windows' | 'linux' | 'macos';
  mouse_triggers?: boolean;
  settings: Settings;
  status: 'active' | 'paused';
  catalog: { pings: CatalogEntry[]; emotes: CatalogEntry[]; trigger_keys?: number[] };
  load_warning: string | null;
}
export interface FieldError { code: string; message: string; field: string | null; request_id: string | null }

export const DIRECTIONS = ['N', 'NE', 'E', 'SE', 'S', 'SW', 'W', 'NW'] as const;
export const DIRECTION_LABELS: Record<(typeof DIRECTIONS)[number], string> = {
  N: 'North',
  NE: 'North-East',
  E: 'East',
  SE: 'South-East',
  S: 'South',
  SW: 'South-West',
  W: 'West',
  NW: 'North-West',
};
export const TRIGGERS: { value: Exclude<Trigger, object>; label: string }[] = [
  { value: 'alt', label: 'Alt' }, { value: 'ctrl', label: 'Ctrl' }, { value: 'shift', label: 'Shift' },
  { value: 'win', label: 'Windows' }, { value: 'caps_lock', label: 'Caps Lock' },
  { value: 'mouse4', label: 'Mouse 4' }, { value: 'mouse5', label: 'Mouse 5' },
];
export function platformTriggers(platform?: BootData['platform'], mouseTriggers = platform !== 'linux') {
  return TRIGGERS.filter(({ value }) =>
    !(!mouseTriggers && (value === 'mouse4' || value === 'mouse5'))
    && !(platform === 'macos' && value === 'caps_lock')
  ).map(option => ({ ...option, label: platform === 'macos' && option.value === 'win' ? 'Command' : platform === 'linux' && option.value === 'win' ? 'Super' : option.label }));
}
export const DEFAULT_PING_WHEEL = ['danger', 'push', 'omw', 'allin', 'assist', 'needvision', 'missing', 'enemyvision'];
export const DEFAULT_EMOTE_WHEEL = ['come_at_me', 'now_see_here', 'does_not_compute', 'sweating_blitzcrank', 'unworthy_cn', 'gg_heart', 'bee_happy', 'bee_mad'];
export function cloneSettings(value: Settings): Settings { return JSON.parse(JSON.stringify(value)) as Settings; }
export function duplicateSlots(values: string[]): Set<number> {
  const seen = new Set<string>(); const duplicates = new Set<number>();
  values.forEach((value, index) => { if (seen.has(value)) duplicates.add(index); else seen.add(value); });
  return duplicates;
}
export function triggerLabel(value: Trigger | null): string {
  if (value === null) return 'Off';
  if (typeof value === 'object') return `Key ${value.key.vk}`;
  return TRIGGERS.find(({ value: key }) => key === value)?.label ?? value;
}
export function formatAssetName(id: string): string {
  return id.replaceAll('_', ' ').replace(/\b\w/g, (letter) => letter.toUpperCase());
}
export function errorFor(errors: Record<string, FieldError>, path: string): string | undefined {
  return errors[path]?.message;
}
export function pageForField(field: string): 'triggers' | 'pings' | 'emotes' {
  if (field.startsWith('emote') || field.startsWith('click_emote')) return 'emotes';
  if (field === 'wheel' || field.startsWith('wheel.') || field.startsWith('wheel_') || field.startsWith('ping') || field.startsWith('click_ping')) return 'pings';
  return 'triggers';
}

/** Annular sector path helper for circular radial wheel SVG (angles in radians, 0 = top/North, clockwise). */
export function sectorPath(cx: number, cy: number, rIn: number, rOut: number, aStart: number, aEnd: number): string {
  const p = (r: number, a: number) => `${cx + r * Math.sin(a)},${cy - r * Math.cos(a)}`;
  const largeArc = Math.abs(aEnd - aStart) > Math.PI ? 1 : 0;
  return `M ${p(rIn, aStart)} L ${p(rOut, aStart)} A ${rOut} ${rOut} 0 ${largeArc} 1 ${p(rOut, aEnd)} L ${p(rIn, aEnd)} A ${rIn} ${rIn} 0 ${largeArc} 0 ${p(rIn, aStart)} Z`;
}
