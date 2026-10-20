<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import brandMark from '../../docs/media/brand/pingloom-mark-mono.svg';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { onMount, tick } from 'svelte';
  import { AlertCircle, Check, ChevronLeft, ChevronRight, Copy, Crosshair, Heart, Keyboard, LoaderCircle, Minus, Moon, Play, RotateCcw, Shield, Square, Sun, Upload, Volume2, X } from '@lucide/svelte';
  import { cloneSettings, DIRECTIONS, DIRECTION_LABELS, duplicateSlots, errorFor, formatAssetName, pageForField, sectorPath, platformTriggers, triggerLabel, type BootData, type CatalogEntry, type FieldError, type Settings, type Trigger } from './lib/settings';

  type ThemeMode = 'light' | 'dark';
  type DarkFlavor = 'frappe' | 'macchiato' | 'mocha';
  type ThemePreference = { family: 'default' | 'catppuccin'; mode: ThemeMode; darkFlavor: DarkFlavor };
  type WheelKind = 'wheel' | 'emote_wheel';
  const THEME_STORAGE_KEY = 'pingloom.ui-theme';
  function readThemePreference(): ThemePreference | null {
    try {
      const value: unknown = JSON.parse(localStorage.getItem(THEME_STORAGE_KEY) ?? 'null');
      if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
      const { family, mode, darkFlavor } = value as Record<string, unknown>;
      if ((family !== 'default' && family !== 'catppuccin') || (mode !== 'light' && mode !== 'dark') || !['frappe', 'macchiato', 'mocha'].includes(String(darkFlavor))) return null;
      return { family, mode, darkFlavor: darkFlavor as DarkFlavor };
    } catch { return null; }
  }
  function systemPrefersDark() {
    try { return window.matchMedia('(prefers-color-scheme: dark)').matches; } catch { return false; }
  }
  function applyTheme(preference: ThemePreference | null, isDark: boolean) {
    const mode = preference?.mode ?? (isDark ? 'dark' : 'light');
    document.documentElement.dataset.themeFamily = preference?.family ?? 'default';
    document.documentElement.dataset.themeMode = mode;
    document.documentElement.dataset.themePalette = preference?.family === 'catppuccin' ? (mode === 'light' ? 'latte' : preference.darkFlavor) : `default-${mode}`;
  }
  const initialThemePreference = readThemePreference();
  const initialOsDark = systemPrefersDark();
  let themePreference = $state<ThemePreference | null>(initialThemePreference);
  let osDark = $state(initialOsDark);
  const resolvedThemeMode = $derived<ThemeMode>(themePreference?.mode ?? (osDark ? 'dark' : 'light'));
  const selectedTheme = $derived(themePreference?.family === 'catppuccin' ? (resolvedThemeMode === 'light' ? 'latte' : themePreference.darkFlavor) : 'default');
  applyTheme(initialThemePreference, initialOsDark);
  $effect(() => applyTheme(themePreference, osDark));

  let boot: BootData | null = $state(null);
  let draft: Settings | null = $state(null);
  let page: 'triggers' | 'pings' | 'emotes' = $state('triggers');
  let loading = $state(true);
  let saving = $state(false);
  let importing = $state(false);
  let importMessage = $state('');
  let sessionAlive = $state(true);
  let loadError = $state('');
  let runtimeWarning = $state('');
  let saveMessage = $state('');
  let errors: Record<string, FieldError> = $state({});
  let requestId = '';
  let formEl: HTMLFormElement | undefined = $state();
  let focusedSlotIndex: number | null = $state(null);
  let isMaximized = $state(false);
  let documentHidden = $state(false);
  let reducedMotion = $state(false);
  let demoStep = $state(2);
  const TRIGGERS = $derived.by(() => platformTriggers(boot?.platform, boot?.mouse_triggers));
  let appearanceError = $state('');
  let appearanceRevision = 0;
  let appearanceQueue = Promise.resolve();
  $effect(() => {
    const session = boot?.session_id;
    const theme = selectedTheme === 'default' ? `default-${resolvedThemeMode}` : selectedTheme;
    if (!session || !sessionAlive) return;
    const revision = ++appearanceRevision;
    // Preserve ordering through rapid changes; an older disk write cannot win last.
    appearanceQueue = appearanceQueue.then(async () => {
      if (revision !== appearanceRevision) return;
      try {
        await invoke('set_appearance', { session_id: session, theme });
        if (revision === appearanceRevision) appearanceError = '';
      } catch (error) {
        if (revision === appearanceRevision) appearanceError = `Could not sync the Wheel theme: ${messageOf(error)} Close and reopen Settings to retry.`;
      }
    });
  });
  const pages = [
    { id: 'triggers' as const, label: 'Triggers', icon: Keyboard },
    { id: 'pings' as const, label: 'Tactical Pings', icon: Crosshair },
    { id: 'emotes' as const, label: 'Emotes', icon: Heart },
  ];
  const dirtyCount = $derived(boot && draft ? (Object.keys(draft) as (keyof Settings)[]).filter(key => JSON.stringify(draft![key]) !== JSON.stringify(boot!.settings[key])).length : 0);
  const activeKind = $derived.by((): WheelKind => page === 'emotes' ? 'emote_wheel' : 'wheel');
  const activeCatalog = $derived.by(() => page === 'emotes' ? 'emotes' as const : 'pings' as const);
  const selectedIndex = $derived(focusedSlotIndex ?? 0);
  const selectedId = $derived(draft?.[activeKind][selectedIndex] ?? '');
  const previewSize = $derived.by(() => page === 'emotes' ? draft?.emote_size_px ?? 160 : draft?.ping_size_px ?? 128);
  let audio: AudioContext | undefined;
  let peekTimer: ReturnType<typeof setTimeout> | undefined;
  let effectTimer: ReturnType<typeof setTimeout> | undefined;
  let peek = $state(false);
  let previewFx = $state<{ id: string; name: string; src: string; size: number; duration: number } | null>(null);
  let announcement = $state('');
  let dragging = $state(false);
  let dragSlot = $state<number | null>(null);
  let dragX = $state(140);
  let dragY = $state(140);
  let turn = $state(0);
  let lastSlot = 0;
  let pointer: { id: number; x: number; y: number; cx: number; cy: number; scale: number } | null = null;
  let suppressClick = false;

  onMount(() => {
    documentHidden = document.hidden;
    let heartbeat: ReturnType<typeof setInterval> | undefined;
    let heartbeatPending = false;
    let unlistenClose: (() => void) | undefined;
    let unlistenResize: (() => void) | undefined;
    let closing = false;
    let disposed = false;
    const colorScheme = window.matchMedia('(prefers-color-scheme: dark)');
    const motion = window.matchMedia('(prefers-reduced-motion: reduce)');
    const onColorSchemeChange = (event: MediaQueryListEvent) => { osDark = event.matches; };
    const onMotionChange = () => { reducedMotion = motion.matches; };
    reducedMotion = motion.matches;
    colorScheme.addEventListener('change', onColorSchemeChange);
    motion.addEventListener('change', onMotionChange);
    const demo = setInterval(() => {
      demoStep = reducedMotion || documentHidden || page !== 'triggers' ? 2 : (demoStep + 1) % 4;
    }, 1150);
    void (async () => {
      try {
        const data = await invoke<BootData>('get_settings_and_catalog', {});
        if (disposed) return;
        boot = data;
        draft = cloneSettings(data.settings);
        runtimeWarning = data.load_warning ?? '';
        heartbeat = setInterval(async () => {
          if (!boot || heartbeatPending || !sessionAlive) return;
          heartbeatPending = true;
          try {
            const session = boot.session_id;
            const runtime = await invoke<{ status: BootData['status']; load_warning: string | null }>('settings_heartbeat', { session_id: session });
            if (!disposed && boot?.session_id === session && sessionAlive) {
              boot.status = runtime.status;
              runtimeWarning = runtime.load_warning ?? '';
            }
          } catch (error) {
            sessionAlive = false;
            loadError = `Settings session expired: ${messageOf(error)}`;
          } finally { heartbeatPending = false; }
        }, 5000);
        const closeListener = await getCurrentWindow().onCloseRequested(async event => {
          event.preventDefault();
          if (closing || !boot) return;
          closing = true;
          try { await invoke('close_settings', { session_id: boot.session_id }); }
          catch (error) { closing = false; loadError = `Could not close Settings: ${messageOf(error)}`; }
        });
        if (disposed) { closeListener(); return; }
        unlistenClose = closeListener;
        try { isMaximized = await getCurrentWindow().isMaximized(); } catch {}
        const resizeListener = await getCurrentWindow().onResized(async () => {
          try { isMaximized = await getCurrentWindow().isMaximized(); } catch {}
        });
        if (disposed) resizeListener(); else unlistenResize = resizeListener;
      } catch (error) { loadError = messageOf(error); }
      finally { loading = false; }
    })();
    return () => {
      disposed = true;
      if (heartbeat) clearInterval(heartbeat);
      clearInterval(demo);
      clearTimeout(peekTimer);
      clearTimeout(effectTimer);
      unlistenClose?.();
      unlistenResize?.();
      colorScheme.removeEventListener('change', onColorSchemeChange);
      motion.removeEventListener('change', onMotionChange);
      void audio?.close().catch(() => {});
    };
  });

  function messageOf(error: unknown): string {
    if (typeof error === 'string') return error;
    if (error && typeof error === 'object' && 'message' in error) return String(error.message);
    return 'The Settings bridge did not respond. Close and reopen Settings to try again.';
  }
  function saveThemePreference(next: ThemePreference) {
    themePreference = next;
    try { localStorage.setItem(THEME_STORAGE_KEY, JSON.stringify(next)); } catch {}
  }
  function selectTheme(value: string) {
    const darkFlavor = themePreference?.darkFlavor ?? 'frappe';
    if (value === 'default') saveThemePreference({ family: 'default', mode: resolvedThemeMode, darkFlavor });
    else if (value === 'latte') saveThemePreference({ family: 'catppuccin', mode: 'light', darkFlavor });
    else if (value === 'frappe' || value === 'macchiato' || value === 'mocha') saveThemePreference({ family: 'catppuccin', mode: 'dark', darkFlavor: value });
  }
  function toggleThemeMode() {
    saveThemePreference({ family: themePreference?.family ?? 'default', mode: resolvedThemeMode === 'dark' ? 'light' : 'dark', darkFlavor: themePreference?.darkFlavor ?? 'frappe' });
  }
  function set(path: keyof Settings, value: unknown) {
    if (!draft) return;
    draft = { ...draft, [path]: value } as Settings;
    delete errors[path];
    saveMessage = '';
  }
  function setWheel(kind: WheelKind, index: number, value: string) {
    if (!draft) return;
    const next = [...draft[kind]];
    next[index] = value;
    set(kind, next);
    delete errors[`${kind}.${index}`];
  }
  function triggerValue(value: Trigger | null): string {
    return value === null ? 'off' : typeof value === 'string' ? value : `key:${value.key.vk}`;
  }
  function assignTrigger(kind: 'trigger' | 'emote_trigger', value: string) {
    if (value === 'off') set(kind, null);
    else if (value.startsWith('key:')) set(kind, { key: { vk: Number(value.slice(4)) } });
    else set(kind, value as Trigger);
  }
  function vkLabel(vk: number): string {
    return vk >= 65 && vk <= 90 || vk >= 48 && vk <= 57 ? String.fromCharCode(vk) : `Key ${vk}`;
  }
  function pathError(path: string) { return errorFor(errors, path); }
  function changePage(next: typeof page) {
    page = next;
    focusedSlotIndex = null;
    turn = 0;
    lastSlot = 0;
    cancelPointer();
    previewFx = null;
    peek = false;
    clearTimeout(effectTimer);
    clearTimeout(peekTimer);
  }
  function onTabKeydown(event: KeyboardEvent, currentIndex: number) {
    let index: number;
    if (event.key === 'ArrowRight' || event.key === 'ArrowDown') index = (currentIndex + 1) % pages.length;
    else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') index = (currentIndex + pages.length - 1) % pages.length;
    else if (event.key === 'Home') index = 0;
    else if (event.key === 'End') index = pages.length - 1;
    else return;
    event.preventDefault();
    changePage(pages[index].id);
    document.getElementById(`tab-${pages[index].id}`)?.focus();
  }
  async function cancel() {
    if (!boot) return;
    try { await invoke('close_settings', { session_id: boot.session_id }); }
    catch (error) { saveMessage = `Could not close Settings: ${messageOf(error)}`; }
  }
  async function handleMinimize() {
    try { await getCurrentWindow().minimize(); } catch (error) { console.error('Could not minimize window:', error); }
  }
  async function handleToggleMaximize() {
    try { await getCurrentWindow().toggleMaximize(); isMaximized = await getCurrentWindow().isMaximized(); }
    catch (error) { console.error('Could not toggle maximize:', error); }
  }
  async function handleDragStart(event: MouseEvent) {
    if (event.button !== 0 || event.detail > 1 || (event.target as HTMLElement | null)?.closest('button, nav, input, select, textarea, [data-no-drag]')) return;
    try { await getCurrentWindow().startDragging(); } catch {}
  }
  function onWheel(event: WheelEvent) {
    if (event.ctrlKey && !(event.target as HTMLElement | null)?.closest('input, textarea, select, [contenteditable="true"]')) event.preventDefault();
  }
  function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape' && boot && !saving) { event.preventDefault(); void cancel(); }
  }
  async function save() {
    if (!boot || !draft || saving || importing || !sessionAlive) return;
    saving = true;
    errors = {};
    saveMessage = '';
    requestId = crypto.randomUUID();
    const candidate = cloneSettings(draft);
    try {
      const result = await invoke<{ request_id: string; settings: Settings; status: BootData['status'] }>('save_settings', { session_id: boot.session_id, request_id: requestId, candidate });
      if (result.request_id !== requestId) throw new Error('Save response did not match this request. Your draft is still open.');
      boot = { ...boot, settings: result.settings, status: result.status };
      draft = cloneSettings(result.settings);
      saveMessage = 'Settings saved successfully.';
      errors = {};
    } catch (error) {
      const rejection = error as Partial<FieldError>;
      if (rejection && typeof rejection === 'object' && typeof rejection.code === 'string') {
        if (rejection.field) {
          errors = { [rejection.field]: rejection as FieldError };
          page = pageForField(rejection.field);
          await tick();
          revealControl(formEl?.querySelector<HTMLElement>(`[data-field="${CSS.escape(rejection.field)}"]`));
        }
        saveMessage = rejection.message ?? 'Settings could not be saved. Your changes are still here.';
      } else saveMessage = `Could not confirm save. Do not retry yet; check Settings state. ${messageOf(error)}`;
    } finally { saving = false; }
  }
  async function importEmote(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file || !boot || !draft || importing || saving || !sessionAlive) return;
    const session = boot.session_id;
    const slot = focusedSlotIndex ?? 0;
    importing = true;
    importMessage = '';
    try {
      if (file.size > 8 * 1024 * 1024) throw new Error('Choose a PNG, JPG, GIF or WebP up to 8 MiB.');
      const data = await new Promise<string>((resolve, reject) => {
        const reader = new FileReader();
        reader.onerror = () => reject(new Error('Could not read this image.'));
        reader.onload = () => resolve(String(reader.result).split(',')[1]);
        reader.readAsDataURL(file);
      });
      const entry = await invoke<CatalogEntry>('import_emote', { session_id: session, data });
      if (!sessionAlive || boot?.session_id !== session || !draft) return;
      if (!boot.catalog.emotes.some(item => item.id === entry.id)) boot.catalog.emotes.push(entry);
      if (!draft.emote_wheel.includes(entry.id)) setWheel('emote_wheel', slot, entry.id);
      importMessage = 'Imported to your local library. Save changes to apply the assignment.';
    } catch (error) {
      if (sessionAlive && boot?.session_id === session) importMessage = error instanceof Error ? error.message : (error as FieldError)?.message || 'Import failed.';
    } finally { importing = false; input.value = ''; }
  }
  function catalog(kind: 'pings' | 'emotes'): CatalogEntry[] { return boot?.catalog[kind] ?? []; }
  function nameFor(kind: 'pings' | 'emotes', id: string) { return catalog(kind).find(item => item.id === id)?.name ?? formatAssetName(id); }
  function previewFor(kind: 'pings' | 'emotes', id: string) { return catalog(kind).find(item => item.id === id)?.preview_url ?? ''; }
  function setMod(mod: number, checked: boolean) {
    if (draft) set('toggle_hotkey', { ...draft.toggle_hotkey, mods: checked ? draft.toggle_hotkey.mods | mod : draft.toggle_hotkey.mods & ~mod });
  }
  function revealControl(target: HTMLElement | null | undefined) {
    if (!target) return;
    const disclosure = target.closest('details');
    if (disclosure) disclosure.open = true;
    target.focus({ preventScroll: true });
    target.scrollIntoView({ behavior: reducedMotion ? 'auto' : 'smooth', block: 'nearest' });
  }
  function selectSlot(index: number) {
    turn += ((index - lastSlot + 12) % 8 - 4) * 45;
    lastSlot = index;
    focusedSlotIndex = index;
  }
  function focusSlot(kind: WheelKind, index: number) {
    if (suppressClick) { suppressClick = false; return; }
    selectSlot(index);
    revealControl(document.getElementById(`picker-${kind}`));
  }
  function hotkeySummary(mods: number, vk: number) {
    const parts: string[] = [];
    if (mods & 1) parts.push('Ctrl');
    if (mods & 2) parts.push('Alt');
    if (mods & 4) parts.push('Shift');
    if (mods & 8) parts.push(boot?.platform === 'macos' ? 'Command' : boot?.platform === 'linux' ? 'Super' : 'Win');
    parts.push(vkLabel(vk));
    return parts.join(' + ');
  }
  function sound(tickOnly = false) {
    if (!draft || draft.volume <= 0) return;
    if (tickOnly ? !draft.tick_sound || draft.muted : page === 'emotes' ? !draft.emote_sound || selectedId.startsWith('custom_') : draft.muted) return;
    try {
      audio ??= new AudioContext();
      void audio.resume().catch(() => {});
      const start = audio.currentTime;
      for (const [frequency, offset, length] of tickOnly ? [[2400, 0, .03]] : [[784, 0, .45], [1175, .06, .6]]) {
        const oscillator = audio.createOscillator();
        const gain = audio.createGain();
        oscillator.type = tickOnly ? 'triangle' : 'sine';
        oscillator.frequency.value = frequency;
        gain.gain.setValueAtTime(.0001, start + offset);
        gain.gain.exponentialRampToValueAtTime(Math.max(.0002, draft.volume / 100 * (tickOnly ? .06 : .14)), start + offset + .006);
        gain.gain.exponentialRampToValueAtTime(.0001, start + offset + length);
        oscillator.connect(gain).connect(audio.destination);
        oscillator.start(start + offset);
        oscillator.stop(start + offset + length + .02);
        oscillator.onended = () => { oscillator.disconnect(); gain.disconnect(); };
      }
    } catch { /* A device-less desktop still has a fully usable visual preview. */ }
  }
  function fire() {
    if (!draft) return;
    clearTimeout(effectTimer);
    clearTimeout(peekTimer);
    peek = false;
    sound();
    const duration = page === 'emotes' ? 3 : draft.ping_duration_s;
    previewFx = { id: crypto.randomUUID(), name: nameFor(activeCatalog, selectedId), src: previewFor(activeCatalog, selectedId), size: previewSize, duration };
    announcement = `${page === 'emotes' ? 'Popped' : 'Sent'} ${previewFx.name}. Local preview only.`;
    effectTimer = setTimeout(() => { previewFx = null; }, duration * 1000 + 520);
  }
  function updateRange(field: keyof Settings, value: number) {
    set(field, value);
    if (field === 'ping_size_px' || field === 'emote_size_px') {
      previewFx = null;
      clearTimeout(effectTimer);
      peek = true;
      clearTimeout(peekTimer);
      peekTimer = setTimeout(() => { peek = false; }, 900);
    }
  }
  function spot(event: PointerEvent) {
    if (event.pointerType === 'touch' || reducedMotion) return;
    const bezel = (event.target as HTMLElement).closest<HTMLElement>('.bezel');
    if (!bezel) return;
    const rect = bezel.getBoundingClientRect();
    bezel.style.setProperty('--mx', `${event.clientX - rect.left}px`);
    bezel.style.setProperty('--my', `${event.clientY - rect.top}px`);
  }
  function pointerDown(event: PointerEvent) {
    if (event.button !== 0 || pointer) return;
    const rect = event.currentTarget instanceof HTMLElement ? event.currentTarget.getBoundingClientRect() : null;
    if (!rect) return;
    suppressClick = false;
    pointer = { id: event.pointerId, x: event.clientX, y: event.clientY, cx: rect.x + rect.width / 2, cy: rect.y + rect.height / 2, scale: 280 / rect.width };
  }
  function pointerMove(event: PointerEvent) {
    if (!pointer || pointer.id !== event.pointerId) return;
    if (!dragging && Math.hypot(event.clientX - pointer.x, event.clientY - pointer.y) < (draft?.drag_threshold_px ?? 12)) return;
    if (!dragging) {
      (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
      dragging = true;
      previewFx = null;
      peek = false;
    }
    const wheelScale = (draft?.wheel_size_percent ?? 100) / 100;
    const x = ((event.clientX - pointer.cx) * pointer.scale) / wheelScale;
    const y = ((event.clientY - pointer.cy) * pointer.scale) / wheelScale;
    const radius = Math.hypot(x, y);
    const slot = radius < 44 ? null : (Math.round(Math.atan2(x, -y) / (Math.PI / 4)) + 8) % 8;
    if (slot !== dragSlot && slot !== null) { selectSlot(slot); sound(true); }
    dragSlot = slot;
    const clamp = radius > 134 ? 134 / radius : 1;
    dragX = 140 + x * clamp;
    dragY = 140 + y * clamp;
  }
  function pointerUp(event: PointerEvent) {
    if (!pointer || pointer.id !== event.pointerId) return;
    if (dragging) {
      pointerMove(event);
      if (dragSlot !== null) fire();
      else announcement = 'Preview cancelled.';
      suppressClick = true;
    } else if (!(event.target as HTMLElement).closest('button')) {
      const wheelScale = (draft?.wheel_size_percent ?? 100) / 100;
      const x = ((event.clientX - pointer.cx) * pointer.scale) / wheelScale;
      const y = ((event.clientY - pointer.cy) * pointer.scale) / wheelScale;
      const radius = Math.hypot(x, y);
      if (radius >= 44 && radius <= 140) focusSlot(activeKind, (Math.round(Math.atan2(x, -y) / (Math.PI / 4)) + 8) % 8);
    }
    const target = event.currentTarget as HTMLElement;
    if (target.hasPointerCapture(event.pointerId)) target.releasePointerCapture(event.pointerId);
    cancelPointer();
  }
  function cancelPointer() { pointer = null; dragging = false; dragSlot = null; }
</script>

<svelte:head><title>Pingloom Settings</title></svelte:head>
<svelte:window onkeydown={onKeydown} onwheel={onWheel} onpointerup={cancelPointer} onpointercancel={cancelPointer} />
<svelte:document onvisibilitychange={() => { documentHidden = document.hidden; if (documentHidden) cancelPointer(); }} />

{#if loading}
  <main class="boot-state" class:window-hidden={documentHidden} aria-live="polite"><LoaderCircle size={24} class="spin" /><span>Loading Settings…</span></main>
{:else if !boot || !draft}
  <main class="bridge-state" role="alert">
    <AlertCircle size={28} class="failure-icon" /><h1>Settings unavailable</h1>
    <p>{loadError || 'The desktop Settings service is unavailable.'}</p>
    <p>No sample configuration was loaded. Your app settings have not been changed.</p>
    <button type="button" class="btn-primary" onclick={() => window.location.reload()}><RotateCcw size={16} /> Try again</button>
  </main>
{:else}
  <div class="desktop-shell" class:window-hidden={documentHidden} role="group" aria-label="Pingloom Settings" onpointermove={spot}>
    <div class="atmosphere" aria-hidden="true"><div class="top-glow"></div><div class="aurora aurora-a"></div><div class="aurora aurora-b"></div></div>
    <header class="desktop-header">
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="header-branding" data-tauri-drag-region onmousedown={handleDragStart} ondblclick={handleToggleMaximize}>
        <div class="app-badge" aria-hidden="true"><span class="brand-symbol" style={`--brand-mark: url("${brandMark}")`}></span></div>
        <div class="app-titles"><span class="app-title-bold">Pingloom</span><span class="app-title-sub">Settings</span></div>
      </div>
      <div class="status-indicator" class:paused={boot.status === 'paused'} title="Gesture engine status; use the toggle shortcut to pause or resume">
        <span class="status-dot"></span><span class="status-label">{boot.status === 'active' ? 'Gestures active' : 'Paused'}</span>
        <span class="status-hotkey">{hotkeySummary(boot.settings.toggle_hotkey.mods, boot.settings.toggle_hotkey.vk)}</span>
      </div>
      <div class="header-drag-filler" data-tauri-drag-region onmousedown={handleDragStart} ondblclick={handleToggleMaximize} aria-hidden="true"></div>
      <div class="theme-controls" data-no-drag>
        <label class="sr-only" for="theme-preset">Color theme</label>
        <select id="theme-preset" class="theme-preset" value={selectedTheme} onchange={e => selectTheme(e.currentTarget.value)}>
          <option value="default">Pingloom Blush</option><option value="latte">Catppuccin Latte</option><option value="frappe">Catppuccin Frappé</option><option value="macchiato">Catppuccin Macchiato</option><option value="mocha">Catppuccin Mocha</option>
        </select>
        <button type="button" class="theme-toggle" aria-label={resolvedThemeMode === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'} aria-pressed={resolvedThemeMode === 'dark'} onclick={toggleThemeMode}>
          {#if resolvedThemeMode === 'dark'}<Sun size={18} />{:else}<Moon size={18} />{/if}
        </button>
      </div>
      <div class="window-controls" aria-label="Window controls" data-no-drag>
        <button type="button" class="window-control-btn btn-minimize" title="Minimize" aria-label="Minimize" onclick={handleMinimize}><Minus size={16} /></button>
        <button type="button" class="window-control-btn btn-maximize" title={isMaximized ? 'Restore Down' : 'Maximize'} aria-label={isMaximized ? 'Restore Down' : 'Maximize'} onclick={handleToggleMaximize}>{#if isMaximized}<Copy size={15} />{:else}<Square size={15} />{/if}</button>
        <button type="button" class="window-control-btn btn-close" title="Close (Cancel changes)" aria-label="Close" onclick={cancel}><X size={17} /></button>
      </div>
    </header>
    <nav class="nav-tabs-wrapper" aria-label="Settings pages" data-no-drag>
      <div class="nav-tabs" role="tablist" style={`--tab-index: ${pages.findIndex(item => item.id === page)}`}>
        <span class="tab-indicator" aria-hidden="true"></span>
        {#each pages as item, index}
          <button type="button" id="tab-{item.id}" data-tab={item.id} role="tab" aria-selected={page === item.id} aria-controls="panel-{item.id}" tabindex={page === item.id ? 0 : -1} class="tab-btn" class:active={page === item.id} onclick={() => changePage(item.id)} onkeydown={event => onTabKeydown(event, index)}><item.icon size={17} strokeWidth={1.75} /><span>{item.label}</span></button>
        {/each}
      </div>
    </nav>
    <main class="desktop-body" id="settings-content">
      {#if loadError || runtimeWarning}<div class="notice-callout warning" role="status"><AlertCircle size={17} /><span>{loadError || runtimeWarning}</span></div>{/if}
      {#if appearanceError}<div class="notice-callout warning" role="status"><AlertCircle size={17} /><span>{appearanceError}</span></div>{/if}
      <form bind:this={formEl} onsubmit={event => { event.preventDefault(); void save(); }}>
        {#if page === 'triggers'}
          <div role="tabpanel" id="panel-triggers" aria-labelledby="tab-triggers" tabindex="0" class="page-panel">
            <section class="bezel trigger-hero">
              <div class="plate">
                <svg class="hero-orbits" viewBox="0 0 440 440" aria-hidden="true"><g fill="none" stroke="currentColor"><circle cx="220" cy="220" r="70" /><circle cx="220" cy="220" r="120" /><circle cx="220" cy="220" r="165" /><circle cx="220" cy="220" r="199" stroke-dasharray="1 7" /></g></svg>
                <h1 class="display"><span class="eyebrow">Triggers</span>Hold <kbd>{triggerLabel(draft.trigger)}</kbd>. Drag.<br /><span class="faint">Release.</span></h1>
                <p class="hero-copy">The wheel opens once the pointer travels {draft.drag_threshold_px} px. Anything shorter counts as a tap.</p>
                <figure class="gesture-banner">
                  <div class="gesture-demo" class:demo-hold={demoStep >= 1} class:demo-release={demoStep >= 2} aria-hidden="true">
                    <div class="demo-part"><kbd class="demo-key">{triggerLabel(draft.trigger)}</kbd><span>1 · Hold</span></div>
                    <div class="demo-part demo-drag"><svg viewBox="0 0 96 44"><path d="M3 22h84m-9-9 9 9-9 9" fill="none" stroke="currentColor" stroke-width="2" stroke-dasharray="4 6" /></svg><span>2 · Drag</span></div>
                    <div class="demo-part"><div class="mini-wheel"><svg viewBox="0 0 280 280"><g fill="var(--core2)" stroke="var(--line)">{#each DIRECTIONS as _, i}<path d={sectorPath(140, 140, 54, 130, (i - .5) * Math.PI / 4 + .025, (i + .5) * Math.PI / 4 - .025)} class:demo-wedge={i === 1} />{/each}</g></svg><img src={previewFor('pings', draft.wheel[1])} alt="" /></div><span>3 · Release</span></div>
                  </div>
                  <figcaption class="sr-only">Hold trigger + left mouse press, drag, and release to select a slice. Release in the center to cancel.</figcaption>
                </figure>
              </div>
            </section>
            <div class="trigger-grid">
              <section class="bezel activation-card"><div class="plate">
                <div class="section-heading"><Keyboard size={19} /><h2>Activation</h2></div><p class="section-desc">Which key opens each wheel.</p>
                {@render TriggerChoices('trigger', 'Ping trigger', draft.trigger)}
                {@render TriggerChoices('emote_trigger', 'Emote trigger', draft.emote_trigger)}
                <p class="hint-text">The emote trigger can't share the ping trigger.</p>
                {@render Slider('threshold', 'drag_threshold_px', 'Drag threshold', draft.drag_threshold_px, 2, 40, 1, 'px', 'How far the pointer moves before the wheel opens. The test wheel on Pings uses it too.')}
              </div></section>
              <section class="bezel shortcut-card"><div class="plate">
                <div class="section-heading"><Shield size={19} /><h2>Global shortcut</h2></div><p class="section-desc">Turns gestures on and off from anywhere.</p>
                <div class="shortcut-preview" aria-label={hotkeySummary(draft.toggle_hotkey.mods, draft.toggle_hotkey.vk)}>{#each hotkeySummary(draft.toggle_hotkey.mods, draft.toggle_hotkey.vk).split(' + ') as key, i}{#if i > 0}<span>+</span>{/if}<kbd>{key}</kbd>{/each}</div>
                <div class="hotkey-editor-row">
                  <div class="modifiers-list">
                    {#each [{ label: 'Ctrl', bit: 1 }, { label: 'Alt', bit: 2 }, { label: 'Shift', bit: 4 }, { label: boot.platform === 'macos' ? 'Command' : boot.platform === 'linux' ? 'Super' : 'Win', bit: 8 }] as mod}
                      <label class="mod-pill"><input type="checkbox" data-field="toggle_hotkey.mods" checked={(draft.toggle_hotkey.mods & mod.bit) !== 0} onchange={e => setMod(mod.bit, e.currentTarget.checked)} /><span>{mod.label}</span></label>
                    {/each}
                  </div>
                  <label class="sr-only" for="toggle-key">Shortcut key</label>
                  <select id="toggle-key" data-field="toggle_hotkey.vk" class:input-error={!!pathError('toggle_hotkey') || !!pathError('toggle_hotkey.vk')} value={draft.toggle_hotkey.vk} onchange={e => set('toggle_hotkey', { ...draft!.toggle_hotkey, vk: Number(e.currentTarget.value) })}>
                    {#each [...new Set([80, ...(boot.catalog.trigger_keys ?? []), draft.toggle_hotkey.vk])] as vk}<option value={vk}>{vkLabel(vk)}</option>{/each}
                  </select>
                </div>
                {#if !(draft.toggle_hotkey.mods & 11)}<p class="shortcut-warning"><AlertCircle size={16} />Add Ctrl, Alt or {boot.platform === 'macos' ? 'Command' : boot.platform === 'linux' ? 'Super' : 'Win'} so the shortcut can't fire while you type.</p>{/if}
                {#if pathError('toggle_hotkey') || pathError('toggle_hotkey.vk') || pathError('toggle_hotkey.mods')}<p class="error-text" role="alert">{pathError('toggle_hotkey') ?? pathError('toggle_hotkey.vk') ?? pathError('toggle_hotkey.mods')}</p>{/if}
                {@render Switch('enabled_on_start', 'Enable gestures at launch', 'Gestures are live as soon as Pingloom starts.', draft.enabled_on_start)}
                {@render Switch('game_check', 'Pause over fullscreen games', "Keeps the game's own cursor when a fullscreen game fills the display.", draft.game_check)}
              </div></section>
            </div>
            <section class="bezel tap-actions"><div class="plate">
              <h2>Tap actions</h2><p class="section-desc">A quick tap of the trigger, with no drag, places something instantly.</p>
              <div class="tap-actions-grid">
                <div class="tap-card">{@render Switch('click_ping', `Tap ${triggerLabel(draft.trigger)} to ping`, 'Drops this ping at the pointer.', draft.click_ping)}
                  {#if draft.click_ping}{@render TapPicker('pings', 'click_ping_id', 'click-ping-id', draft.click_ping_id)}{/if}
                </div>
                <div class="tap-card">{@render Switch('emote_click', `Tap ${draft.emote_trigger ? triggerLabel(draft.emote_trigger) : 'the emote key'} to emote`, 'Pops this emote at the pointer.', draft.emote_click)}
                  {#if draft.emote_click}{@render TapPicker('emotes', 'click_emote_id', 'click-emote-id', draft.click_emote_id)}{/if}
                </div>
              </div>
            </div></section>
          </div>
        {:else}
          {@const kind = activeKind}
          {@const choices = catalog(activeCatalog)}
          {@const values = draft[kind]}
          <div role="tabpanel" id="panel-{page}" aria-labelledby="tab-{page}" tabindex="0" class="page-panel">
            <div class="wheel-hero">
              <div><h1 class="display"><span class="eyebrow">{page === 'pings' ? 'Tactical Pings' : 'Emotes'}</span>{#if page === 'pings'}Eight directions.<br /><span class="faint">One flick.</span>{:else}Say it<br /><span class="faint">without typing.</span>{/if}</h1>
                <p class="hero-copy">{#if page === 'pings'}In game, hold <kbd>{triggerLabel(draft.trigger)}</kbd> and flick toward a slice. Here, drag across the wheel to feel it, or click a slice to change what it sends.{:else if draft.emote_trigger && triggerValue(draft.emote_trigger) !== 'off'}In game, hold <kbd>{triggerLabel(draft.emote_trigger)}</kbd>, flick and release. Drag across the wheel to try it, and bring your own art for any slice.{:else}The emote wheel is off. Pick an emote trigger on the Triggers page to use it in game.{/if}</p>
              </div>
              <dl class="wheel-stats"><div><dt>Slices</dt><dd>8</dd></div><div><dt>{page === 'pings' ? 'Size' : 'Library'}</dt><dd>{page === 'pings' ? draft.ping_size_px : choices.length}{#if page === 'pings'}<span>px</span>{/if}</dd></div><div><dt>{page === 'pings' ? 'On screen' : 'Size'}</dt><dd>{page === 'pings' ? draft.ping_duration_s.toFixed(1) : draft.emote_size_px}<span>{page === 'pings' ? 's' : 'px'}</span></dd></div></dl>
            </div>
            <div class="two-column-workspace">
              <div class="column-preview">
                <section class="bezel wheel-card" aria-label="Wheel preview"><div class="plate">
                  <div class="dot-grid" aria-hidden="true"></div>
                  <div class="preview-panel-header"><span class="preview-title"><span class="live-dot"></span>Live wheel</span><span class="preview-subtitle">Drag to test · click to edit</span></div>
                  {@render WheelRadialPreview(kind, choices, values)}
                  <div class="display-sliders">
                    {@render Slider('wheel-size', 'wheel_size_percent', 'Radar size', draft.wheel_size_percent, 60, 150, 1, '%')}
                    {#if page === 'pings'}
                      {@render Slider('ping-size', 'ping_size_px', 'Ping size', draft.ping_size_px, 60, 220, 1, 'px')}
                      {@render Slider('ping-duration', 'ping_duration_s', 'Duration', draft.ping_duration_s, 1, 8, .1, 's')}
                    {:else}{@render Slider('emote-size', 'emote_size_px', 'Emote size', draft.emote_size_px, 80, 300, 1, 'px')}{/if}
                  </div>
                  <div class="test-row"><p>Release in the middle to cancel, like in game.</p><button type="button" class="test-button" onclick={fire}><span><Play size={15} fill="currentColor" /></span>Test {nameFor(activeCatalog, selectedId)}</button></div>
                  <p class="hint-text preview-disclaimer">Local preview · synthesized sound, not the bundled clip.</p>
                </div></section>
                {#if page === 'pings'}
                  <section class="bezel sound-card"><div class="plate"><div class="section-heading"><Volume2 size={19} /><h2>Sound</h2></div><p class="section-desc">You'll hear it when you test.</p>
                    {@render Switch('tick_sound', 'Selection ticks', 'A soft tick each time the pointer crosses into a new slice.', draft.tick_sound)}
                    {@render Switch('muted', 'Mute ping sounds', 'Pings still show on screen, silently.', draft.muted)}
                    {@render Slider('volume', 'volume', 'Effects volume', draft.volume, 0, 100, 1, '%', '', draft.muted)}
                  </div></section>
                {:else}
                  <section class="bezel import-card"><div class="plate"><h2>Your own expressions.</h2><p class="section-desc">Bring something only you would send.</p>
                    <label class="import-dropzone" class:disabled={importing || saving || !sessionAlive} for="custom-emote-file"><span class="upload-icon">{#if importing}<LoaderCircle size={23} class="spin" />{:else}<Upload size={23} />{/if}</span><span><strong>{importing ? 'Importing…' : 'Import a custom emote'}</strong><small>PNG, JPG, GIF or WebP</small></span>
                      <input id="custom-emote-file" type="file" accept="image/png,image/jpeg,image/gif,image/webp,.png,.jpg,.jpeg,.gif,.webp" onchange={importEmote} disabled={importing || saving || !sessionAlive} aria-describedby="custom-emote-help custom-emote-message" />
                    </label>
                    <p id="custom-emote-help" class="hint-text">Up to 8 MiB and 2048 px. Animated images play for 3 s. It lands on the selected slice. The library holds 16 images, 32 MiB in total. Custom emotes play without sound for now.</p>
                    <p id="custom-emote-message" class="import-message" role="status">{importing ? 'Importing…' : importMessage}</p>
                    {@render Switch('emote_sound', 'Emote sounds', 'Play the bundled sound with each emote.', draft.emote_sound)}
                  </div></section>
                {/if}
              </div>
              <section class="bezel slice-editor" aria-label="Slice editor"><div class="plate">
                <div class="slice-toolbar"><span class="slice-chip">Slice {DIRECTIONS[selectedIndex]} · {DIRECTION_LABELS[DIRECTIONS[selectedIndex]]}</span><div><button type="button" class="round-button" aria-label="Previous slice" onclick={() => selectSlot((selectedIndex + 7) % 8)}><ChevronLeft size={20} /></button><button type="button" class="round-button" aria-label="Next slice" onclick={() => selectSlot((selectedIndex + 1) % 8)}><ChevronRight size={20} /></button></div></div>
                {@render AssetPicker(kind, choices, values)}
                <details class="slot-details"><summary>All 8 assignments</summary>{@render SlotEditorTable(kind, choices, values)}</details>
                {#if pathError(kind)}<p class="error-text" role="alert">{pathError(kind)}</p>{/if}
              </div></section>
            </div>
          </div>
        {/if}
      </form>
    </main>
    <footer class="desktop-footer">
      <div class="footer-status" aria-live="polite">
        {#if saving}<div class="status-msg saving"><LoaderCircle size={16} class="spin" /><span>Saving settings…</span></div>
        {:else if Object.keys(errors).length || saveMessage && saveMessage !== 'Settings saved successfully.'}<div class="status-msg error"><AlertCircle size={16} /><span>{saveMessage || 'Please correct validation errors above.'}</span></div>
        {:else if saveMessage}<div class="status-msg success"><Check size={16} /><span>{saveMessage}</span></div>
        {:else}<div class="status-msg info" class:dirty={dirtyCount > 0}><span class="footer-dot"></span><span>{dirtyCount ? `${dirtyCount} unsaved change${dirtyCount === 1 ? '' : 's'}` : 'Everything is saved'}</span></div>{/if}
      </div>
      <div class="footer-actions"><button type="button" class="btn-footer-ghost" disabled={saving} onclick={cancel} title="Close and discard unsaved changes (Esc)">Close <kbd class="kbd-subtle">Esc</kbd></button><button type="button" class="btn-footer-primary" disabled={saving || importing || !sessionAlive} onclick={save}><span class="save-icon">{#if saving}<LoaderCircle size={17} class="spin" />{:else}<Check size={18} />{/if}</span>{saving ? 'Saving…' : 'Save changes'}</button></div>
    </footer>
  </div>
{/if}

{#snippet TriggerChoices(field: 'trigger' | 'emote_trigger', label: string, value: Trigger | null)}
  {@const options = [...(field === 'emote_trigger' ? [{ value: 'off', label: 'Off' }] : []), ...TRIGGERS, ...[...new Set([...(boot?.catalog.trigger_keys ?? []), ...(value && typeof value !== 'string' ? [value.key.vk] : [])])].map(vk => ({ value: `key:${vk}`, label: vkLabel(vk) }))]}
  <fieldset class="trigger-choices"><legend>{label}</legend><div class="trigger-chips">
    {#each options as option, i}<label class="trigger-chip"><input id="{field}-{i}" type="radio" name={field} data-field={field} value={option.value} checked={triggerValue(value) === option.value} disabled={field === 'emote_trigger' && option.value === triggerValue(draft!.trigger)} onchange={() => assignTrigger(field, option.value)} aria-describedby={pathError(field) ? `${field}-error` : undefined} /><span>{option.label}</span></label>{/each}
  </div>{#if pathError(field)}<p id="{field}-error" class="error-text" role="alert">{pathError(field)}</p>{/if}</fieldset>
{/snippet}

{#snippet Switch(field: keyof Settings, label: string, help: string, checked: boolean)}
  <label class="switch-row"><span class="checkbox-labels"><strong>{label}</strong><small>{help}</small></span><input type="checkbox" class="switch" data-field={field} {checked} onchange={event => set(field, event.currentTarget.checked)} /></label>
  {#if pathError(field)}<p class="error-text" role="alert">{pathError(field)}</p>{/if}
{/snippet}

{#snippet Slider(id: string, field: keyof Settings, label: string, value: number, min: number, max: number, step: number, unit: string, help = '', disabled = false)}
  <div class="slider-group" class:disabled>
    <div class="slider-heading"><label for={id}>{label}</label><output for={id}>{step < 1 ? value.toFixed(1) : value}<span>{unit}</span></output></div>
    <div class="slider-track" style={`--fraction: ${(value - min) / (max - min)}`}><input {id} data-field={field} type="range" {min} {max} {step} {value} {disabled} aria-invalid={!!pathError(field)} aria-describedby={help || pathError(field) ? `${id}-help` : undefined} oninput={e => updateRange(field, Number(e.currentTarget.value))} /><span class="slider-rail"></span><span class="slider-fill"></span><span class="slider-thumb"></span></div>
    {#if help || pathError(field)}<p id="{id}-help" class:hint-text={!pathError(field)} class:error-text={!!pathError(field)}>{pathError(field) || help}</p>{/if}
  </div>
{/snippet}

{#snippet TapPicker(kind: 'pings' | 'emotes', field: 'click_ping_id' | 'click_emote_id', id: string, value: string)}
  <div class="tap-picker-row"><div class="tap-thumb"><img src={previewFor(kind, value)} alt="" /></div><label class="sr-only" for={id}>Tap {kind === 'pings' ? 'ping' : 'emote'} asset</label><select {id} data-field={field} {value} onchange={e => set(field, e.currentTarget.value)}>{#each catalog(kind) as choice}<option value={choice.id}>{choice.name}</option>{/each}</select></div>
  {#if pathError(field)}<p class="error-text" role="alert">{pathError(field)}</p>{/if}
{/snippet}

{#snippet AssetPicker(kind: WheelKind, choices: CatalogEntry[], values: string[])}
  {@const index = selectedIndex}
  {@const selected = choices.find(choice => choice.id === values[index])}
  <div class="asset-picker" id="picker-{kind}" tabindex="-1">
    <div class="assignment-heading" aria-live="polite"><div class="assignment-art">{#if selected?.preview_url}<img src={selected.preview_url} alt="" />{/if}</div><h2>{selected?.name ?? 'Choose an asset'}</h2><p>{kind === 'wheel' ? 'Sends' : 'Pops up'} when you release toward {DIRECTION_LABELS[DIRECTIONS[index]].toLowerCase()}.</p></div>
    <div class="library-heading"><h3>Library</h3><span>{choices.length} {kind === 'wheel' ? 'pings' : 'emotes'}{#if kind === 'emote_wheel'} · {choices.filter(c => c.id.startsWith('custom_')).length} custom{/if}</span></div>
    <div class="asset-library" aria-label="Available {kind === 'wheel' ? 'pings' : 'emotes'}">
      {#each choices as choice}
        <button type="button" class="asset-tile" class:assigned={values[index] === choice.id} aria-pressed={values[index] === choice.id} aria-label="Assign {choice.name} to {DIRECTION_LABELS[DIRECTIONS[index]]}" onclick={() => { focusedSlotIndex = index; setWheel(kind, index, choice.id); }}>
          <img src={choice.preview_url} alt="" loading="lazy" /><span>{choice.name}</span>
          {#if values[index] === choice.id}<span class="asset-check"><Check size={12} /></span>{:else if values.includes(choice.id)}<span class="used-at">{values.map((id, i) => id === choice.id ? DIRECTIONS[i] : '').filter(Boolean).join(' · ')}</span>{/if}
        </button>
      {/each}
    </div>
  </div>
{/snippet}

{#snippet WheelRadialPreview(kind: WheelKind, choices: CatalogEntry[], values: string[])}
  {@const duplicates = duplicateSlots(values)}
  {@const wheelScale = (draft?.wheel_size_percent ?? 100) / 100}
  <div class="radial-wheel" style={`--wheel-scale: ${wheelScale}`} class:dragging class:previewing={!!previewFx || peek} role="region" aria-label="Interactive radial wheel representation" onpointerdown={pointerDown} onpointermove={pointerMove} onpointerup={pointerUp} onpointercancel={cancelPointer} onlostpointercapture={cancelPointer}>
    <div class="wheel-stage">
      <div class="wheel-glow" style={`--turn: ${turn}deg`} aria-hidden="true"><div></div></div>
      <svg class="radial-svg" viewBox="0 0 280 280" aria-hidden="true" focusable="false">
        <defs><radialGradient id="wheel-surface" cx="140" cy="140" r="134" gradientUnits="userSpaceOnUse"><stop offset="38%" stop-color="var(--wedge-in)" /><stop offset="100%" stop-color="var(--wedge-out)" /></radialGradient></defs>
        <circle cx="140" cy="140" r="139" class="wheel-ticks" stroke-width="3" stroke-dasharray=".6 5.465" /><circle cx="140" cy="140" r="139" class="wheel-major-ticks" stroke-width="7" stroke-dasharray="1.6 107.57" stroke-dashoffset=".8" />
        {#each DIRECTIONS as _, i}<path d={sectorPath(140, 140, 54, 134, (i - .5) * Math.PI / 4 + .025, (i + .5) * Math.PI / 4 - .025)} class="radial-wedge" class:wedge-duplicate={duplicates.has(i)} />{/each}
        <circle cx="140" cy="140" r="44" class="wheel-hub" class:cancelling={dragging && dragSlot === null} />
        <g class="wheel-highlight" class:cancelling={dragging && dragSlot === null} style={`transform: rotate(${turn}deg)`}><path d={sectorPath(140, 140, 54, 134, -.5 * Math.PI / 4 + .025, .5 * Math.PI / 4 - .025)} /><path class="highlight-arc" d="M92.84 17.78A131 131 0 0 1 187.16 17.78" /><path class="highlight-inner" d="M123.16 99.35A44 44 0 0 1 156.84 99.35" /></g>
        <g class="wheel-direction-labels">{#each DIRECTIONS as dir, i}<text x={140 + 124 * Math.sin(i * Math.PI / 4)} y={140 - 124 * Math.cos(i * Math.PI / 4)}>{dir}</text>{/each}</g>
        <text x="140" y="135" class="hub-title">RELEASE</text><text x="140" y="147" class="hub-sub">to cancel</text>
        {#if dragging}<g class="drag-line"><line x1="140" y1="140" x2={dragX} y2={dragY} /><circle cx="140" cy="140" r="3.5" /><circle cx={dragX} cy={dragY} r="9" opacity=".2" /><circle cx={dragX} cy={dragY} r="4.5" /></g>{/if}
      </svg>
      {#each DIRECTIONS as direction, i}
        {@const choice = choices.find(c => c.id === values[i])}
        <button type="button" class="wheel-node pos-{direction.toLowerCase()}" class:duplicate={duplicates.has(i)} class:focused={selectedIndex === i} aria-pressed={selectedIndex === i} aria-label="{direction} ({DIRECTION_LABELS[direction]}): {choice?.name ?? formatAssetName(values[i] ?? '')}" style="left: {(50 + 32.14 * Math.sin(i * Math.PI / 4)).toFixed(3)}%; top: {(50 - 32.14 * Math.cos(i * Math.PI / 4)).toFixed(3)}%;" onclick={() => focusSlot(kind, i)}>
          {#if choice?.preview_url}{#key choice.id}<img src={choice.preview_url} alt="" class="node-img" draggable="false" />{/key}{:else}<span>{direction}</span>{/if}
          {#if duplicates.has(i)}<span class="node-duplicate-pill">Repeats</span>{/if}
        </button>
      {/each}
    </div>
    {#if peek}<div class="size-peek" aria-hidden="true"><div style={`width: ${previewSize}px; height: ${previewSize}px`}><img src={previewFor(activeCatalog, selectedId)} alt="" /><span>{previewSize} px on screen</span></div></div>{/if}
    {#if previewFx}{#key previewFx.id}<div class="preview-effect" style={`--effect-size: ${previewFx.size}px; --effect-duration: ${previewFx.duration}s; --effect-total: ${previewFx.duration + .5}s`} aria-hidden="true"><span class="effect-ring"></span><span class="effect-ring second"></span><svg viewBox="0 0 100 100"><circle cx="50" cy="50" r="48" /><circle class="countdown" cx="50" cy="50" r="48" /></svg><img src={previewFx.src} alt="" /><span class="effect-caption">{previewFx.name} · {previewFx.duration.toFixed(1)} s</span></div>{/key}{/if}
  </div>
  <p class="sr-only" aria-live="polite">{announcement}</p>
{/snippet}

{#snippet SlotEditorTable(kind: WheelKind, choices: CatalogEntry[], values: string[])}
  {@const duplicates = duplicateSlots(values)}
  <div class="slots-table" role="group" aria-label="8 directional slice assignments">
    {#each DIRECTIONS as direction, index}
      {@const choice = choices.find(c => c.id === values[index])}
      {@const error = pathError(`${kind}.${index}`)}
      <div class="slot-row" class:row-focused={focusedSlotIndex === index} class:row-error={!!error}>
        <span class="dir-code">{direction}</span><div class="slot-thumb-cell">{#if choice?.preview_url}<img src={choice.preview_url} alt="" />{/if}</div>
        <div class="slot-select-cell"><label class="sr-only" for="{kind}-{index}">{direction} slice assignment</label><select id="{kind}-{index}" data-field="{kind}.{index}" class:input-error={!!error} value={values[index]} onfocus={() => selectSlot(index)} onchange={e => setWheel(kind, index, e.currentTarget.value)}>{#each choices as option}<option value={option.id}>{option.name}</option>{/each}</select>
          {#if error}<span class="error-text" role="alert">{error}</span>{:else if duplicates.has(index)}<span class="duplicate-text">Repeats {formatAssetName(values[index])}</span>{/if}
        </div>
      </div>
    {/each}
  </div>
{/snippet}
