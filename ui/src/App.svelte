<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import brandMark from '../../docs/media/brand/pingloom-mark-mono.svg';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { onMount, tick } from 'svelte';
  import {
    Activity,
    AlertCircle,
    Check,
    Copy,
    Crosshair,
    Gamepad2,
    Heart,
    Info,
    Keyboard,
    LoaderCircle,
    Minus,
    MousePointer2,
    Music2,
    RotateCcw,
    Save,
    Shield,
    Square,
    Sun,
    Moon,
    Volume2,
    X,
  } from '@lucide/svelte';
  import {
    cloneSettings,
    DIRECTIONS,
    DIRECTION_LABELS,
    duplicateSlots,
    errorFor,
    formatAssetName,
    pageForField,
    sectorPath,
    platformTriggers,
    type BootData,
    type CatalogEntry,
    type FieldError,
    type Settings,
    type Trigger,
  } from './lib/settings';

  type ThemeMode = 'light' | 'dark';
  type ThemeFamily = 'default' | 'catppuccin';
  type DarkFlavor = 'frappe' | 'macchiato' | 'mocha';
  type ThemePreference = { family: ThemeFamily; mode: ThemeMode; darkFlavor: DarkFlavor };

  const THEME_STORAGE_KEY = 'pingloom.ui-theme';

  function readThemePreference(): ThemePreference | null {
    try {
      const value: unknown = JSON.parse(localStorage.getItem(THEME_STORAGE_KEY) ?? 'null');
      if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
      const { family, mode, darkFlavor } = value as Record<string, unknown>;
      if ((family !== 'default' && family !== 'catppuccin') || (mode !== 'light' && mode !== 'dark') || !['frappe', 'macchiato', 'mocha'].includes(String(darkFlavor))) return null;
      return { family, mode, darkFlavor: darkFlavor as DarkFlavor };
    } catch {
      return null;
    }
  }

  function systemPrefersDark() {
    try { return window.matchMedia('(prefers-color-scheme: dark)').matches; } catch { return false; }
  }

  function applyTheme(preference: ThemePreference | null, isDark: boolean) {
    const mode = preference?.mode ?? (isDark ? 'dark' : 'light');
    const palette = preference?.family === 'catppuccin' ? (mode === 'light' ? 'latte' : preference.darkFlavor) : `default-${mode}`;
    document.documentElement.dataset.themeFamily = preference?.family ?? 'default';
    document.documentElement.dataset.themeMode = mode;
    document.documentElement.dataset.themePalette = palette;
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
  let saveMessage = $state('');
  let errors: Record<string, FieldError> = $state({});
  let requestId = '';
  let formEl: HTMLFormElement | undefined = $state();
  let focusedSlotIndex: number | null = $state(null);
  let isMaximized = $state(false);
  let documentHidden = $state(false);
  const TRIGGERS = $derived.by(() => platformTriggers(boot?.platform));

  const pages = [
    { id: 'triggers' as const, label: 'Triggers', icon: Keyboard },
    { id: 'pings' as const, label: 'Tactical Pings', icon: Crosshair },
    { id: 'emotes' as const, label: 'Emotes', icon: Heart },
  ];

  onMount(() => {
    documentHidden = document.hidden;
    let heartbeat: ReturnType<typeof setInterval> | undefined;
    let heartbeatPending = false;
    let unlistenClose: (() => void) | undefined;
    let unlistenResize: (() => void) | undefined;
    let closing = false;
    let colorScheme: MediaQueryList | undefined;
    const onColorSchemeChange = (event: MediaQueryListEvent) => { osDark = event.matches; };
    try {
      colorScheme = window.matchMedia('(prefers-color-scheme: dark)');
      colorScheme.addEventListener('change', onColorSchemeChange);
    } catch {}

    void (async () => {
      try {
        boot = await invoke<BootData>('get_settings_and_catalog', {});
        draft = cloneSettings(boot.settings);
        if (boot.load_warning) loadError = boot.load_warning;

        heartbeat = setInterval(async () => {
          if (!boot || heartbeatPending || !sessionAlive) return;
          heartbeatPending = true;
          try {
            const session = boot.session_id;
            const runtimeStatus = await invoke<BootData['status']>('settings_heartbeat', { session_id: session });
            if (boot?.session_id === session && sessionAlive) boot.status = runtimeStatus;
          } catch (error) {
            sessionAlive = false;
            loadError = `Settings session expired: ${messageOf(error)}`;
          } finally {
            heartbeatPending = false;
          }
        }, 5000);

        unlistenClose = await getCurrentWindow().onCloseRequested(async (event) => {
          event.preventDefault();
          if (closing || !boot) return;
          closing = true;
          try {
            await invoke('close_settings', { session_id: boot.session_id });
          } catch (error) {
            closing = false;
            loadError = `Could not close Settings: ${messageOf(error)}`;
          }
        });

        try {
          isMaximized = await getCurrentWindow().isMaximized();
        } catch {}

        unlistenResize = await getCurrentWindow().onResized(async () => {
          try {
            isMaximized = await getCurrentWindow().isMaximized();
          } catch {}
        });
      } catch (error) {
        loadError = messageOf(error);
      } finally {
        loading = false;
      }
    })();

    return () => {
      if (heartbeat) clearInterval(heartbeat);
      unlistenClose?.();
      unlistenResize?.();
      colorScheme?.removeEventListener('change', onColorSchemeChange);
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
    saveThemePreference({
      family: themePreference?.family ?? 'default',
      mode: resolvedThemeMode === 'dark' ? 'light' : 'dark',
      darkFlavor: themePreference?.darkFlavor ?? 'frappe',
    });
  }

  function set(path: keyof Settings, value: unknown) {
    if (!draft) return;
    draft = { ...draft, [path]: value } as Settings;
    delete errors[path];
    saveMessage = '';
  }

  function setWheel(kind: 'wheel' | 'emote_wheel', index: number, value: string) {
    if (!draft) return;
    const next = [...draft[kind]];
    next[index] = value;
    set(kind, next);
    delete errors[`${kind === 'wheel' ? 'wheel' : 'emote_wheel'}.${index}`];
  }

  function triggerValue(value: Trigger | null): string {
    if (value === null) return 'off';
    return typeof value === 'string' ? value : `key:${value.key.vk}`;
  }

  function assignTrigger(kind: 'trigger' | 'emote_trigger', value: string) {
    if (value === 'off') set(kind, null);
    else if (value.startsWith('key:')) set(kind, { key: { vk: Number(value.slice(4)) } });
    else set(kind, value as Trigger);
  }

  function vkLabel(vk: number): string {
    if (vk >= 65 && vk <= 90) return String.fromCharCode(vk);
    if (vk >= 48 && vk <= 57) return String.fromCharCode(vk);
    return `Key ${vk}`;
  }

  function pathError(path: string): string | undefined {
    return errorFor(errors, path);
  }

  function changePage(next: typeof page) {
    page = next;
    focusedSlotIndex = null;
  }

  function onTabKeydown(event: KeyboardEvent, currentIndex: number) {
    if (event.key === 'ArrowRight' || event.key === 'ArrowDown') {
      event.preventDefault();
      const nextIndex = (currentIndex + 1) % pages.length;
      changePage(pages[nextIndex].id);
      document.getElementById(`tab-${pages[nextIndex].id}`)?.focus();
    } else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') {
      event.preventDefault();
      const prevIndex = (currentIndex - 1 + pages.length) % pages.length;
      changePage(pages[prevIndex].id);
      document.getElementById(`tab-${pages[prevIndex].id}`)?.focus();
    } else if (event.key === 'Home') {
      event.preventDefault();
      changePage(pages[0].id);
      document.getElementById(`tab-${pages[0].id}`)?.focus();
    } else if (event.key === 'End') {
      event.preventDefault();
      changePage(pages[pages.length - 1].id);
      document.getElementById(`tab-${pages[pages.length - 1].id}`)?.focus();
    }
  }

  async function cancel() {
    if (!boot) return;
    try {
      await invoke('close_settings', { session_id: boot.session_id });
    } catch (error) {
      saveMessage = `Could not close Settings: ${messageOf(error)}`;
    }
  }

  async function handleMinimize() {
    try {
      await getCurrentWindow().minimize();
    } catch (err) {
      console.error('Could not minimize window:', err);
    }
  }

  async function handleToggleMaximize() {
    try {
      await getCurrentWindow().toggleMaximize();
      isMaximized = await getCurrentWindow().isMaximized();
    } catch (err) {
      console.error('Could not toggle maximize:', err);
    }
  }

  async function handleDragStart(event: MouseEvent) {
    if (event.button !== 0 || event.detail > 1) return;
    const target = event.target as HTMLElement | null;
    if (target?.closest('button, nav, input, select, textarea, [data-no-drag]')) return;
    try {
      await getCurrentWindow().startDragging();
    } catch {}
  }

  function onWheel(event: WheelEvent) {
    if (event.ctrlKey) {
      const target = event.target as HTMLElement | null;
      if (!target?.closest('input, textarea, select, [contenteditable="true"]')) {
        event.preventDefault();
      }
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape' && boot && !saving) {
      event.preventDefault();
      void cancel();
    }
  }

  async function save() {
    if (!boot || !draft || saving || importing || !sessionAlive) return;
    saving = true;
    errors = {};
    saveMessage = '';
    requestId = crypto.randomUUID();

    // Use clean clone without Proxy wrappers for native serialization
    const candidate = cloneSettings(draft);

    try {
      const result = await invoke<{ request_id: string; settings: Settings; status: BootData['status'] }>('save_settings', {
        session_id: boot.session_id,
        request_id: requestId,
        candidate,
      });

      if (result.request_id !== requestId) {
        throw new Error('Save response did not match this request. Your draft is still open.');
      }

      boot = { ...boot, settings: result.settings, status: result.status };
      draft = cloneSettings(result.settings);
      saveMessage = 'Settings saved successfully.';
      errors = {};
    } catch (error) {
      const rejection = error as Partial<FieldError>;
      if (rejection && typeof rejection === 'object' && typeof rejection.code === 'string') {
        if (rejection.field) {
          errors = { [rejection.field]: rejection as FieldError };
          const field = rejection.field;
          page = pageForField(field);
          await tick();
          revealControl(formEl?.querySelector<HTMLElement>(`[data-field="${CSS.escape(field)}"]`));
        }
        saveMessage = rejection.message ?? 'Settings could not be saved. Your changes are still here.';
      } else {
        saveMessage = `Could not confirm save. Do not retry yet; check Settings state. ${messageOf(error)}`;
      }
    } finally {
      saving = false;
    }
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
    } finally {
      importing = false;
      input.value = '';
    }
  }

  function catalog(kind: 'pings' | 'emotes'): CatalogEntry[] {
    return boot?.catalog[kind] ?? [];
  }

  function nameFor(kind: 'pings' | 'emotes', id: string): string {
    return catalog(kind).find((item) => item.id === id)?.name ?? formatAssetName(id);
  }

  function previewFor(kind: 'pings' | 'emotes', id: string): string {
    return catalog(kind).find((item) => item.id === id)?.preview_url ?? '';
  }

  function setMod(mod: number, checked: boolean) {
    if (!draft) return;
    set('toggle_hotkey', {
      ...draft.toggle_hotkey,
      mods: checked ? draft.toggle_hotkey.mods | mod : draft.toggle_hotkey.mods & ~mod,
    });
  }

  function revealControl(target: HTMLElement | null | undefined) {
    if (!target) return;
    const disclosure = target.closest('details');
    if (disclosure) disclosure.open = true;
    target.focus({ preventScroll: true });
    target.scrollIntoView({
      behavior: window.matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth',
      block: 'nearest',
    });
  }

  function focusSlot(kind: 'wheel' | 'emote_wheel', index: number) {
    focusedSlotIndex = index;
    revealControl(document.getElementById(`picker-${kind}`));
  }

  function hotkeySummary(mods: number, vk: number): string {
    const parts: string[] = [];
    if (mods & 1) parts.push('Ctrl');
    if (mods & 2) parts.push('Alt');
    if (mods & 4) parts.push('Shift');
    if (mods & 8) parts.push(boot?.platform === 'macos' ? 'Command' : boot?.platform === 'linux' ? 'Super' : 'Win');
    parts.push(vkLabel(vk));
    return parts.join(' + ');
  }
</script>

<svelte:head>
  <title>Pingloom Settings</title>
</svelte:head>
<svelte:window onkeydown={onKeydown} onwheel={onWheel} />
<svelte:document onvisibilitychange={() => documentHidden = document.hidden} />

{#if loading}
  <main class="boot-state" class:window-hidden={documentHidden} aria-live="polite">
    <LoaderCircle size={24} class="spin" />
    <span>Loading Settings…</span>
  </main>
{:else if !boot || !draft}
  <main class="bridge-state" role="alert">
    <div class="failure-icon"><AlertCircle size={28} /></div>
    <h1>Settings unavailable</h1>
    <p>{loadError || 'The desktop Settings service is unavailable.'}</p>
    <p class="bridge-detail">No sample configuration was loaded. Your app settings have not been changed.</p>
    <button type="button" class="btn btn-primary" onclick={() => window.location.reload()}>
      <RotateCcw size={14} /> Try again
    </button>
  </main>
{:else}
  <div class="desktop-shell" class:window-hidden={documentHidden}>
    <!-- Native window controls stay separate from Settings navigation. -->
    <header class="desktop-header">
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="header-branding"
        data-tauri-drag-region
        onmousedown={handleDragStart}
        ondblclick={handleToggleMaximize}
      >
        <div class="app-badge" aria-hidden="true">
          <span class="brand-symbol" aria-hidden="true" style={`--brand-mark: url("${brandMark}")`}></span>
        </div>
        <div class="app-titles">
          <span class="app-title-bold">Pingloom</span>
          <span class="app-title-sub">Settings</span>
        </div>
        <div
          class="status-indicator"
          class:paused={boot.status === 'paused'}
          title="Gesture engine status"
        >
          <span class="status-dot"></span>
          <span class="status-label">
            {boot.status === 'active' ? 'Gestures active' : 'Paused'}
          </span>
        </div>
      </div>

      <div class="theme-controls" data-no-drag>
        <label class="sr-only" for="theme-preset">Color theme</label>
        <select id="theme-preset" class="theme-preset" value={selectedTheme} onchange={(event) => selectTheme(event.currentTarget.value)}>
          <option value="default">Default Sapphire</option>
          <option value="latte">Catppuccin Latte</option>
          <option value="frappe">Catppuccin Frappé</option>
          <option value="macchiato">Catppuccin Macchiato</option>
          <option value="mocha">Catppuccin Mocha</option>
        </select>
        <button
          type="button"
          class="theme-toggle"
          aria-label={resolvedThemeMode === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
          title={resolvedThemeMode === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
          aria-pressed={resolvedThemeMode === 'dark'}
          onclick={toggleThemeMode}
        >
          {#if resolvedThemeMode === 'dark'}<Sun size={16} aria-hidden="true" />{:else}<Moon size={16} aria-hidden="true" />{/if}
        </button>
      </div>

      <!-- Draggable titlebar spacer -->
      <div
        class="header-drag-filler"
        data-tauri-drag-region
        onmousedown={handleDragStart}
        ondblclick={handleToggleMaximize}
        aria-hidden="true"
      ></div>

      <!-- Native Desktop Window Controls: Minimize, Maximize/Restore, Close -->
      <div class="window-controls" aria-label="Window controls" data-no-drag>
        <button
          type="button"
          class="window-control-btn btn-minimize"
          title="Minimize"
          aria-label="Minimize"
          onclick={handleMinimize}
        >
          <Minus size={13} strokeWidth={2} />
        </button>
        <button
          type="button"
          class="window-control-btn btn-maximize"
          title={isMaximized ? 'Restore Down' : 'Maximize'}
          aria-label={isMaximized ? 'Restore Down' : 'Maximize'}
          onclick={handleToggleMaximize}
        >
          {#if isMaximized}
            <Copy size={12} strokeWidth={2} />
          {:else}
            <Square size={12} strokeWidth={2} />
          {/if}
        </button>
        <button
          type="button"
          class="window-control-btn btn-close"
          title="Close (Cancel changes)"
          aria-label="Close"
          onclick={cancel}
        >
          <X size={13} strokeWidth={2} />
        </button>
      </div>
    </header>

    <nav class="nav-tabs-wrapper" aria-label="Settings pages" data-no-drag>
      <div class="nav-tabs" role="tablist">
        {#each pages as item, index}
          <button
            type="button"
            id="tab-{item.id}"
            data-tab={item.id}
            role="tab"
            aria-selected={page === item.id}
            aria-controls="panel-{item.id}"
            tabindex={page === item.id ? 0 : -1}
            class="tab-btn"
            class:active={page === item.id}
            onclick={() => changePage(item.id)}
            onkeydown={(e) => onTabKeydown(e, index)}
          >
            <item.icon size={16} strokeWidth={1.75} />
            <span>{item.label}</span>
          </button>
        {/each}
      </div>
    </nav>

    <!-- Main Settings Form Area -->
    <main class="desktop-body" id="settings-content">
      {#if loadError}
        <div class="notice-callout warning" role="status">
          <AlertCircle size={15} />
          <span>{loadError}</span>
        </div>
      {/if}

      <form bind:this={formEl} onsubmit={(e) => { e.preventDefault(); void save(); }}>
        <!-- Triggers Page -->
        {#if page === 'triggers'}
          <div role="tabpanel" id="panel-triggers" aria-labelledby="tab-triggers" tabindex="0" class="page-panel">
            <div class="panel-header">
              <h1>Triggers & Shortcuts</h1>
              <p>Configure mouse/keyboard activation keys, hold drag threshold, and global toggle hotkey.</p>
            </div>

            <!-- Gesture Instruction Callout Banner -->
            <div class="gesture-banner">
              <Info size={16} class="banner-icon" />
              <div class="banner-content">
                <strong>Gesture activation:</strong>
                <span>Hold trigger + left mouse press, drag, and release to select a slice. Release in center to cancel.</span>
              </div>
            </div>

            <!-- Group 1: Gesture Triggers -->
            <section class="config-card" aria-labelledby="heading-gesture-triggers">
              <div class="card-header">
                <Keyboard size={15} />
                <h2 id="heading-gesture-triggers">Gesture triggers</h2>
              </div>
              <div class="card-body">
                <div class="grid-2col">
                  <div class="form-group">
                    <label for="ping-trigger">Ping trigger key</label>
                    <select
                      id="ping-trigger"
                      data-field="trigger"
                      class:input-error={pathError('trigger')}
                      value={triggerValue(draft.trigger)}
                      onchange={(e) => assignTrigger('trigger', e.currentTarget.value)}
                    >
                      {#each TRIGGERS as option}
                        <option value={option.value}>{option.label}</option>
                      {/each}
                      {#each boot.catalog.trigger_keys ?? [] as vk}
                        <option value="key:{vk}">Key {vkLabel(vk)}</option>
                      {/each}
                      {#if typeof draft.trigger !== 'string' && !(boot.catalog.trigger_keys ?? []).includes(draft.trigger.key.vk)}
                        <option value="key:{draft.trigger.key.vk}">Key {vkLabel(draft.trigger.key.vk)} (current)</option>
                      {/if}
                    </select>
                    {#if typeof draft.trigger !== 'string'}
                      <span class="hint-text">Custom virtual key {vkLabel(draft.trigger.key.vk)}</span>
                    {/if}
                    {#if pathError('trigger')}
                      <span class="error-text" role="alert">{pathError('trigger')}</span>
                    {/if}
                  </div>

                  <div class="form-group">
                    <label for="emote-trigger">Emote trigger key</label>
                    <select
                      id="emote-trigger"
                      data-field="emote_trigger"
                      class:input-error={pathError('emote_trigger')}
                      value={triggerValue(draft.emote_trigger)}
                      onchange={(e) => assignTrigger('emote_trigger', e.currentTarget.value)}
                    >
                      <option value="off">Off (Disabled)</option>
                      {#each TRIGGERS as option}
                        <option value={option.value}>{option.label}</option>
                      {/each}
                      {#each boot.catalog.trigger_keys ?? [] as vk}
                        <option value="key:{vk}">Key {vkLabel(vk)}</option>
                      {/each}
                      {#if draft.emote_trigger && typeof draft.emote_trigger !== 'string' && !(boot.catalog.trigger_keys ?? []).includes(draft.emote_trigger.key.vk)}
                        <option value="key:{draft.emote_trigger.key.vk}">Key {vkLabel(draft.emote_trigger.key.vk)} (current)</option>
                      {/if}
                    </select>
                    {#if draft.emote_trigger && typeof draft.emote_trigger !== 'string'}
                      <span class="hint-text">Custom virtual key {vkLabel(draft.emote_trigger.key.vk)}</span>
                    {/if}
                    {#if pathError('emote_trigger')}
                      <span class="error-text" role="alert">{pathError('emote_trigger')}</span>
                    {/if}
                  </div>
                </div>

                <div class="grid-2col pt-separator">
                  <div class="form-group">
                    <label for="threshold">
                      Drag threshold <span class="unit-tag">px</span>
                    </label>
                    <div class="input-with-unit">
                      <input
                        id="threshold"
                        data-field="drag_threshold_px"
                        type="number"
                        min="2"
                        max="40"
                        step="1"
                        class:input-error={pathError('drag_threshold_px')}
                        bind:value={draft.drag_threshold_px}
                      />
                      <span class="unit-suffix">pixels</span>
                    </div>
                    <span class="hint-text">Distance mouse must move before radial wheel appears (2–40 px).</span>
                    {#if pathError('drag_threshold_px')}
                      <span class="error-text" role="alert">{pathError('drag_threshold_px')}</span>
                    {/if}
                  </div>

                  <div class="form-group checkbox-align">
                    <label class="checkbox-row">
                      <input
                        type="checkbox"
                        data-field="enabled_on_start"
                        class="checkbox checkbox-sm checkbox-primary"
                        bind:checked={draft.enabled_on_start}
                      />
                      <span class="checkbox-labels">
                        <strong>Enable gestures at launch</strong>
                        <small>Start running gesture interception automatically when Pingloom starts.</small>
                      </span>
                    </label>
                  </div>
                </div>
              </div>
            </section>

            <!-- Group 2: Toggle Shortcut -->
            <section class="config-card" aria-labelledby="heading-toggle-hotkey">
              <div class="card-header">
                <Shield size={15} />
                <h2 id="heading-toggle-hotkey">Global toggle shortcut</h2>
              </div>
              <div class="card-body">
                <p class="section-desc">Pause or resume gesture interception system-wide without opening Settings.</p>

                <div class="hotkey-editor-row">
                  <div class="modifiers-group">
                    <span class="sublabel">Modifiers:</span>
                    <div class="modifiers-list">
                      {#each [{ label: 'Ctrl', bit: 1 }, { label: 'Alt', bit: 2 }, { label: 'Shift', bit: 4 }, { label: boot.platform === 'macos' ? 'Command' : boot.platform === 'linux' ? 'Super' : 'Windows', bit: 8 }] as mod}
                        <label class="mod-pill">
                          <input
                            type="checkbox"
                            data-field="toggle_hotkey.mods"
                            class="checkbox checkbox-xs checkbox-primary"
                            checked={(draft.toggle_hotkey.mods & mod.bit) !== 0}
                            onchange={(e) => setMod(mod.bit, e.currentTarget.checked)}
                          />
                          <span>{mod.label}</span>
                        </label>
                      {/each}
                    </div>
                  </div>

                  <div class="hotkey-key-group">
                    <label for="toggle-key">Key:</label>
                    <select
                      id="toggle-key"
                      data-field="toggle_hotkey.vk"
                      class:input-error={pathError('toggle_hotkey') || pathError('toggle_hotkey.vk')}
                      value={draft.toggle_hotkey.vk}
                      onchange={(e) => set('toggle_hotkey', { ...draft!.toggle_hotkey, vk: Number(e.currentTarget.value) })}
                    >
                      <option value="80">P</option>
                      {#each boot.catalog.trigger_keys ?? [] as vk}
                        <option value={vk}>{vkLabel(vk)}</option>
                      {/each}
                      {#if !boot.catalog.trigger_keys?.includes(draft.toggle_hotkey.vk) && draft.toggle_hotkey.vk !== 80}
                        <option value={draft.toggle_hotkey.vk}>{vkLabel(draft.toggle_hotkey.vk)} (current)</option>
                      {/if}
                    </select>
                  </div>

                  <div class="hotkey-preview-badge">
                    <span class="preview-label">Active shortcut:</span>
                    <kbd class="kbd-badge">{hotkeySummary(draft.toggle_hotkey.mods, draft.toggle_hotkey.vk)}</kbd>
                  </div>
                </div>

                {#if pathError('toggle_hotkey') || pathError('toggle_hotkey.vk')}
                  <span class="error-text" role="alert">{pathError('toggle_hotkey') ?? pathError('toggle_hotkey.vk')}</span>
                {/if}

                <div class="pt-separator">
                  <label class="checkbox-row">
                    <input
                      type="checkbox"
                      data-field="game_check"
                      class="checkbox checkbox-sm checkbox-primary"
                      bind:checked={draft.game_check}
                    />
                    <span class="checkbox-labels">
                      <strong>Pause over fullscreen games</strong>
                      <small>Preserves native game cursor when a fullscreen game occupies the display.</small>
                    </span>
                  </label>
                </div>
              </div>
            </section>

            <!-- Group 3: Trigger Tap Actions -->
            <section class="config-card" aria-labelledby="heading-tap-actions">
              <div class="card-header">
                <Gamepad2 size={15} />
                <h2 id="heading-tap-actions">Trigger tap actions</h2>
              </div>
              <div class="card-body">
                <p class="section-desc">Instantly place a ping or emote when tapping the trigger key without dragging the cursor.</p>

                <div class="tap-actions-grid">
                  <!-- Tap Ping -->
                  <div class="tap-card">
                    <label class="checkbox-row">
                      <input
                        type="checkbox"
                        data-field="click_ping"
                        class="checkbox checkbox-sm checkbox-primary"
                        bind:checked={draft.click_ping}
                      />
                      <span class="checkbox-labels">
                        <strong>Trigger-tap ping</strong>
                        <small>Place instant ping on quick key release without drag.</small>
                      </span>
                    </label>

                    {#if draft.click_ping}
                      <div class="tap-picker-row">
                        <div class="tap-thumb">
                          <img src={previewFor('pings', draft.click_ping_id)} alt="" />
                        </div>
                        <div class="tap-select-wrap">
                          <label class="sr-only" for="click-ping-id">Tap ping asset</label>
                          <select
                            id="click-ping-id"
                            data-field="click_ping_id"
                            class:input-error={pathError('click_ping_id')}
                            value={draft.click_ping_id}
                            onchange={(e) => set('click_ping_id', e.currentTarget.value)}
                          >
                            {#each catalog('pings') as choice}
                              <option value={choice.id}>{choice.name}</option>
                            {/each}
                          </select>
                          {#if pathError('click_ping_id')}
                            <span class="error-text" role="alert">{pathError('click_ping_id')}</span>
                          {/if}
                        </div>
                      </div>
                    {/if}
                  </div>

                  <!-- Tap Emote -->
                  <div class="tap-card">
                    <label class="checkbox-row">
                      <input
                        type="checkbox"
                        data-field="emote_click"
                        class="checkbox checkbox-sm checkbox-primary"
                        bind:checked={draft.emote_click}
                      />
                      <span class="checkbox-labels">
                        <strong>Trigger-tap emote</strong>
                        <small>Show instant emote on quick key release without drag.</small>
                      </span>
                    </label>

                    {#if draft.emote_click}
                      <div class="tap-picker-row">
                        <div class="tap-thumb">
                          <img src={previewFor('emotes', draft.click_emote_id)} alt="" />
                        </div>
                        <div class="tap-select-wrap">
                          <label class="sr-only" for="click-emote-id">Tap emote asset</label>
                          <select
                            id="click-emote-id"
                            data-field="click_emote_id"
                            class:input-error={pathError('click_emote_id')}
                            value={draft.click_emote_id}
                            onchange={(e) => set('click_emote_id', e.currentTarget.value)}
                          >
                            {#each catalog('emotes') as choice}
                              <option value={choice.id}>{choice.name}</option>
                            {/each}
                          </select>
                          {#if pathError('click_emote_id')}
                            <span class="error-text" role="alert">{pathError('click_emote_id')}</span>
                          {/if}
                        </div>
                      </div>
                    {/if}
                  </div>
                </div>
              </div>
            </section>
          </div>

        <!-- Pings Page -->
        {:else if page === 'pings'}
          <div role="tabpanel" id="panel-pings" aria-labelledby="tab-pings" tabindex="0" class="page-panel">
            <div class="panel-header">
              <h1>Tactical Pings</h1>
              <p>Configure the 8-directional ping wheel slices, visual scale, and sound feedback.</p>
            </div>

            <!-- Two-Column Desktop Layout: Preview + Controls -->
            <div class="two-column-workspace">
              <!-- Left Column: Interactive Wheel Preview -->
              <div class="column-preview">
                <div class="preview-panel-header">
                  <span class="preview-title">Wheel Preview</span>
                  <span class="preview-subtitle">Click slice to edit</span>
                </div>

                {@render WheelRadialPreview('wheel', catalog('pings'), draft.wheel, (idx) => focusSlot('wheel', idx))}
              </div>

              <!-- Right Column: 8 Slot Editors & Parameters -->
              <div class="column-controls">
                <section class="config-card" aria-labelledby="heading-ping-slices">
                  <div class="card-header">
                    <Crosshair size={15} />
                    <h2 id="heading-ping-slices">Choose a ping</h2>
                  </div>
                  <div class="card-body">
                    {@render AssetPicker('wheel', catalog('pings'), draft.wheel)}
                    <details class="slot-details">
                      <summary>All 8 assignments</summary>
                      {@render SlotEditorTable('wheel', catalog('pings'), draft.wheel, (idx, val) => setWheel('wheel', idx, val))}
                    </details>
                    {#if pathError('wheel')}
                      <span class="error-text pt-2" role="alert">{pathError('wheel')}</span>
                    {/if}
                  </div>
                </section>

                <!-- Ping Overlay Display Settings -->
                <section class="config-card" aria-labelledby="heading-ping-display">
                  <div class="card-header">
                    <MousePointer2 size={15} />
                    <h2 id="heading-ping-display">Overlay display</h2>
                  </div>
                  <div class="card-body">
                    <div class="grid-2col">
                      <div class="form-group">
                        <label for="ping-size">
                          Ping size <span class="unit-tag">px</span>
                        </label>
                        <input
                          id="ping-size"
                          data-field="ping_size_px"
                          type="number"
                          min="60"
                          max="220"
                          step="1"
                          class:input-error={pathError('ping_size_px')}
                          bind:value={draft.ping_size_px}
                        />
                        <span class="hint-text">Size of ping marker on desktop (60–220 px).</span>
                        {#if pathError('ping_size_px')}
                          <span class="error-text" role="alert">{pathError('ping_size_px')}</span>
                        {/if}
                      </div>

                      <div class="form-group">
                        <label for="ping-duration">
                          Duration <span class="unit-tag">sec</span>
                        </label>
                        <input
                          id="ping-duration"
                          data-field="ping_duration_s"
                          type="number"
                          min="1"
                          max="8"
                          step="0.1"
                          class:input-error={pathError('ping_duration_s')}
                          bind:value={draft.ping_duration_s}
                        />
                        <span class="hint-text">Seconds ping remains visible (1.0–8.0 s).</span>
                        {#if pathError('ping_duration_s')}
                          <span class="error-text" role="alert">{pathError('ping_duration_s')}</span>
                        {/if}
                      </div>
                    </div>
                  </div>
                </section>

                <!-- Sound & Feedback -->
                <section class="config-card" aria-labelledby="heading-ping-sound">
                  <div class="card-header">
                    <Volume2 size={15} />
                    <h2 id="heading-ping-sound">Sound & feedback</h2>
                  </div>
                  <div class="card-body">
                    <div class="checkboxes-stack">
                      <label class="checkbox-row">
                        <input
                          type="checkbox"
                          data-field="tick_sound"
                          class="checkbox checkbox-sm checkbox-primary"
                          bind:checked={draft.tick_sound}
                        />
                        <span class="checkbox-labels">
                          <strong>Selection ticks</strong>
                          <small>Play subtle audio tick when cursor crosses between wheel slices.</small>
                        </span>
                      </label>

                      <label class="checkbox-row">
                        <input
                          type="checkbox"
                          data-field="muted"
                          class="checkbox checkbox-sm checkbox-primary"
                          bind:checked={draft.muted}
                        />
                        <span class="checkbox-labels">
                          <strong>Mute ping sounds</strong>
                          <small>Suppress all ping audio effects completely.</small>
                        </span>
                      </label>
                    </div>

                    <div class="volume-slider-group pt-separator">
                      <div class="volume-slider-header">
                        <label for="volume">Effects volume</label>
                        <span class="volume-value-badge">{draft.volume}%</span>
                      </div>
                      <input
                        id="volume"
                        data-field="volume"
                        type="range"
                        min="0"
                        max="100"
                        step="1"
                        class="range range-xs range-primary"
                        bind:value={draft.volume}
                      />
                      {#if pathError('volume')}
                        <span class="error-text" role="alert">{pathError('volume')}</span>
                      {/if}
                    </div>
                  </div>
                </section>
              </div>
            </div>
          </div>

        <!-- Emotes Page -->
        {:else}
          <div role="tabpanel" id="panel-emotes" aria-labelledby="tab-emotes" tabindex="0" class="page-panel">
            <div class="panel-header">
              <h1>Emotes</h1>
              <p>Assign bundled or custom emotes to the 8-directional wheel and configure display scale and audio.</p>
            </div>

            <!-- Two-Column Desktop Layout: Preview + Controls -->
            <div class="two-column-workspace">
              <!-- Left Column: Interactive Wheel Preview -->
              <div class="column-preview">
                <div class="preview-panel-header">
                  <span class="preview-title">Emote Wheel Preview</span>
                  <span class="preview-subtitle">Click slice to edit</span>
                </div>

                {@render WheelRadialPreview('emote_wheel', catalog('emotes'), draft.emote_wheel, (idx) => focusSlot('emote_wheel', idx))}
              </div>

              <!-- Right Column: 8 Slot Editors & Parameters -->
              <div class="column-controls">
                <section class="config-card" aria-labelledby="heading-emote-slices">
                  <div class="card-header">
                    <Heart size={15} />
                    <h2 id="heading-emote-slices">Choose an emote</h2>
                  </div>
                  <div class="card-body">
                    <label for="custom-emote-file" class="field-label">Import custom emote</label>
                    <input id="custom-emote-file" type="file" accept="image/png,image/jpeg,image/gif,image/webp,.png,.jpg,.jpeg,.gif,.webp" class="file-input file-input-sm w-full" onchange={importEmote} disabled={importing || saving || !sessionAlive} aria-describedby="custom-emote-help custom-emote-message" />
                    <p id="custom-emote-help" class="field-help">PNG / JPG / GIF / WebP · up to 8 MiB / 2048px. GIF/WebP animate for 3s; reduced motion uses a still. Library: 16 images / 32 MiB. No custom sound yet.</p>
                    <p id="custom-emote-message" class="field-help" role="status">{importing ? 'Importing…' : importMessage}</p>
                    {@render AssetPicker('emote_wheel', catalog('emotes'), draft.emote_wheel)}
                    <details class="slot-details">
                      <summary>All 8 assignments</summary>
                      {@render SlotEditorTable('emote_wheel', catalog('emotes'), draft.emote_wheel, (idx, val) => setWheel('emote_wheel', idx, val))}
                    </details>
                    {#if pathError('emote_wheel')}
                      <span class="error-text pt-2" role="alert">{pathError('emote_wheel')}</span>
                    {/if}
                  </div>
                </section>

                <!-- Emote Display Settings -->
                <section class="config-card" aria-labelledby="heading-emote-display">
                  <div class="card-header">
                    <MousePointer2 size={15} />
                    <h2 id="heading-emote-display">Display size</h2>
                  </div>
                  <div class="card-body">
                    <div class="form-group max-w-sm">
                      <label for="emote-size">
                        Emote size <span class="unit-tag">px</span>
                      </label>
                      <input
                        id="emote-size"
                        data-field="emote_size_px"
                        type="number"
                        min="80"
                        max="300"
                        step="1"
                        class:input-error={pathError('emote_size_px')}
                        bind:value={draft.emote_size_px}
                      />
                      <span class="hint-text">Size of emote animation overlay (80–300 px).</span>
                      {#if pathError('emote_size_px')}
                        <span class="error-text" role="alert">{pathError('emote_size_px')}</span>
                      {/if}
                    </div>
                  </div>
                </section>

                <!-- Emote Audio Settings -->
                <section class="config-card" aria-labelledby="heading-emote-audio">
                  <div class="card-header">
                    <Music2 size={15} />
                    <h2 id="heading-emote-audio">Emote audio</h2>
                  </div>
                  <div class="card-body">
                    <label class="checkbox-row">
                      <input
                        type="checkbox"
                        data-field="emote_sound"
                        class="checkbox checkbox-sm checkbox-primary"
                        bind:checked={draft.emote_sound}
                      />
                      <span class="checkbox-labels">
                        <strong>Play bundled emote sounds</strong>
                        <small>Play accompanying sound clip when showing emotes that have audio.</small>
                      </span>
                    </label>
                  </div>
                </section>
              </div>
            </div>
          </div>
        {/if}
      </form>
    </main>

    <!-- Persistent Bottom Command Bar -->
    <footer class="desktop-footer">
      <div class="footer-status" aria-live="polite">
        {#if saving}
          <div class="status-msg saving">
            <LoaderCircle size={14} class="spin" />
            <span>Saving settings…</span>
          </div>
        {:else if Object.keys(errors).length > 0}
          <div class="status-msg error">
            <AlertCircle size={14} />
            <span>{saveMessage || 'Please correct validation errors above.'}</span>
          </div>
        {:else if saveMessage}
          <div class="status-msg success">
            <Check size={14} />
            <span>{saveMessage}</span>
          </div>
        {:else}
          <div class="status-msg info">
            <Shield size={14} />
            <span>Settings apply immediately on save.</span>
          </div>
        {/if}
      </div>

      <div class="footer-actions">
        <button
          type="button"
          class="btn-footer-ghost"
          disabled={saving}
          onclick={cancel}
          title="Close and discard unsaved changes (Esc)"
        >
          <span>Close</span>
          <kbd class="kbd-subtle">Esc</kbd>
        </button>

        <button
          type="button"
          class="btn-footer-primary"
          disabled={saving || importing || !sessionAlive}
          onclick={save}
        >
          {#if saving}
            <LoaderCircle size={14} class="spin" />
            <span>Saving…</span>
          {:else}
            <Save size={14} />
            <span>Save changes</span>
          {/if}
        </button>
      </div>
    </footer>
  </div>
{/if}

<!-- lolPing-inspired visual catalog: choose a slice, then assign artwork. -->
{#snippet AssetPicker(kind: 'wheel' | 'emote_wheel', choices: CatalogEntry[], values: string[])}
  {@const index = focusedSlotIndex ?? 0}
  {@const selected = choices.find(choice => choice.id === values[index])}
  <div class="asset-picker" id="picker-{kind}" tabindex="-1">
    <div class="assignment-heading" aria-live="polite">
      <span class="assignment-direction">{DIRECTIONS[index]}</span>
      <div><strong>{selected?.name ?? 'Choose an asset'}</strong><p>Choose a slice on the wheel, then an image below.</p></div>
    </div>
    <div class="asset-library" aria-label="Available {kind === 'wheel' ? 'pings' : 'emotes'}">
      {#each choices as choice}
        <button type="button" class="asset-tile" class:assigned={values[index] === choice.id}
          aria-pressed={values[index] === choice.id}
          aria-label="Assign {choice.name} to {DIRECTION_LABELS[DIRECTIONS[index]]}"
          onclick={() => { focusedSlotIndex = index; setWheel(kind, index, choice.id); }}>
          <img src={choice.preview_url} alt="" loading="lazy" />
          <span>{choice.name}</span>
          {#if values[index] === choice.id}<Check size={12} class="asset-check" />{/if}
        </button>
      {/each}
    </div>
  </div>
{/snippet}

<!-- Snippet: Radial Wheel Preview -->
{#snippet WheelRadialPreview(kind: 'wheel' | 'emote_wheel', choices: CatalogEntry[], values: string[], onSelectSlot: (idx: number) => void)}
  {@const duplicates = duplicateSlots(values)}
  <div class="radial-wheel-box">
    <div class="radial-wheel" role="region" aria-label="Interactive radial wheel representation">
      <!-- Upstream-inspired Annular 8-segment decorative SVG geometry -->
      <svg class="radial-svg" viewBox="0 0 280 280" aria-hidden="true" focusable="false">
        <defs>
          <radialGradient id="wheel-surface" cx="50%" cy="50%" r="50%">
            <stop offset="35%" stop-color="var(--wheel-surface-start)" />
            <stop offset="100%" stop-color="var(--wheel-surface-end)" />
          </radialGradient>
        </defs>
        <circle cx="140" cy="140" r="134" class="radial-outer-boundary" />
        <circle cx="140" cy="140" r="50" class="radial-inner-boundary" />
        {#each DIRECTIONS as dir, i}
          {@const aStart = ((i - 0.5) * Math.PI) / 4 + 0.025}
          {@const aEnd = ((i + 0.5) * Math.PI) / 4 - 0.025}
          {@const isDup = duplicates.has(i)}
          {@const isFoc = focusedSlotIndex === i}
          <path
            d={sectorPath(140, 140, 50, 134, aStart, aEnd)}
            class="radial-wedge"
            class:wedge-duplicate={isDup}
            class:wedge-focused={isFoc || (focusedSlotIndex === null && i === 0)}
          />
        {/each}
      </svg>

      <!-- Center Cancel Zone -->
      <div class="wheel-center-zone">
        <span class="center-tag">{kind === 'wheel' ? 'PING' : 'EMOTE'}</span>
        <span class="center-sub">Cancel zone</span>
      </div>

      <!-- 8 Directional Slice Buttons in True Circular Ring Geometry (proportional percentages) -->
      {#each DIRECTIONS as direction, i}
        {@const choice = choices.find((c) => c.id === values[i])}
        {@const isDuplicate = duplicates.has(i)}
        {@const isFocused = focusedSlotIndex === i}
        {@const angle = (i * Math.PI) / 4}
        {@const pctX = ((140 + 92 * Math.sin(angle)) / 280) * 100}
        {@const pctY = ((140 - 92 * Math.cos(angle)) / 280) * 100}
        <button
          type="button"
          class="wheel-node pos-{direction.toLowerCase()}"
          class:duplicate={isDuplicate}
          class:focused={isFocused || (focusedSlotIndex === null && i === 0)}
          aria-pressed={(focusedSlotIndex ?? 0) === i}
          style="left: {pctX.toFixed(3)}%; top: {pctY.toFixed(3)}%;"
          onclick={() => onSelectSlot(i)}
          title="{direction} ({DIRECTION_LABELS[direction]}): {choice?.name ?? formatAssetName(values[i] ?? '')}"
        >
          <div class="node-art-wrap">
            {#if choice?.preview_url}
              {#key choice.id}
                <img src={choice.preview_url} alt="" class="node-img" />
              {/key}
            {:else}
              <span class="node-fallback">{direction}</span>
            {/if}
            <span class="node-dir-badge">{direction}</span>
          </div>

          <span class="node-name-label">{choice?.name ?? formatAssetName(values[i] ?? '')}</span>

          {#if isDuplicate}
            <span class="node-duplicate-pill" title="Repeats an asset used in another slice">Repeats</span>
          {/if}
        </button>
      {/each}
    </div>
  </div>
{/snippet}

<!-- Snippet: 8 Slot Table / List Editor -->
{#snippet SlotEditorTable(kind: 'wheel' | 'emote_wheel', choices: CatalogEntry[], values: string[], onChange: (index: number, value: string) => void)}
  {@const duplicates = duplicateSlots(values)}
  <div class="slots-table" role="group" aria-label="8 directional slice assignments">
    {#each DIRECTIONS as direction, index}
      {@const choice = choices.find((c) => c.id === values[index])}
      {@const isDuplicate = duplicates.has(index)}
      {@const errorMsg = pathError(`${kind}.${index}`)}
      {@const isFocused = focusedSlotIndex === index}

      <div class="slot-row" class:row-focused={isFocused} class:row-error={!!errorMsg}>
        <!-- Direction Badge -->
        <div class="slot-dir-cell">
          <span class="dir-code">{direction}</span>
          <span class="dir-name">{DIRECTION_LABELS[direction]}</span>
        </div>

        <!-- Asset Thumbnail Preview -->
        <div class="slot-thumb-cell">
          {#if choice?.preview_url}
            <img src={choice.preview_url} alt="" class="slot-thumb-img" />
          {:else}
            <div class="slot-thumb-placeholder">{direction}</div>
          {/if}
        </div>

        <!-- Asset Select Dropdown -->
        <div class="slot-select-cell">
          <label class="sr-only" for="{kind}-{index}">{direction} slice assignment</label>
          <select
            id="{kind}-{index}"
            data-field="{kind}.{index}"
            class:input-error={!!errorMsg}
            value={values[index]}
            onfocus={() => { focusedSlotIndex = index; }}
            onblur={() => { if (focusedSlotIndex === index) focusedSlotIndex = null; }}
            onchange={(e) => onChange(index, e.currentTarget.value)}
          >
            {#each choices as opt}
              <option value={opt.id}>{opt.name}</option>
            {/each}
          </select>

          {#if errorMsg}
            <span class="error-text" role="alert">{errorMsg}</span>
          {:else if isDuplicate}
            <span class="duplicate-text">Repeats {formatAssetName(values[index])}</span>
          {/if}
        </div>
      </div>
    {/each}
  </div>
{/snippet}
