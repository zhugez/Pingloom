# Desktop UI design

## Scope
One Tauri 2 local Settings WebView using Svelte 5/Vite/Tailwind/daisyUI and `@lucide/svelte`; keep native ping/emote overlays, hooks/replay, animation/audio and tray (Win32 retained; experimental GTK/X11 and Quartz/AppKit adapters). No website, server, remote assets or WebView overlay. This supersedes the previous Rust-only/no-WebView restriction for Settings alone.

## Desktop utility architecture
Designed as a compact, authentic desktop utility rather than a website, landing page, or SaaS dashboard.

### 1. App shell & header
- **Fixed header & native desktop titlebar (46px)**: Compact desktop utility titlebar built frameless (`decorations(false)`) with Windows 11 Mica material vibrancy. Contains the Pingloom app badge, title (`Pingloom Settings`), active gesture status pill (`Gestures active` / `Paused`), integrated segmented tab bar (`Triggers`, `Tactical Pings`, `Emotes`), draggable titlebar regions (`data-tauri-drag-region` on branding and spacer, with double-click to toggle maximize), non-draggable navigation tabs (`data-no-drag`), and native desktop caption controls (Minimize, Maximize / Restore Down, and Close).
  - Window caption controls: Compact Windows-style buttons using Lucide icons (`Minus`, `Square` / `Copy`, `X`) with keyboard focus and active hover states (red hover for Close).
  - Generation-aware Close: The custom Close button strictly invokes `close_settings` via existing IPC (`cancel()`), preserving the generation-aware native lifecycle without bypassing to `destroy()` or unvalidated window close APIs.
  - Windows 11 Mica vibrancy & safe opaque fallback: Created with Tauri's built-in `EffectsBuilder::new().effect(Effect::Mica).build()` and transparent WebView background (`transparent(true)`). Content panels and controls maintain solid, readable opaque surfaces, while CSS background tinting (`--win-shell-bg: rgba(..., 0.85)`) ensures seamless readability even when Mica is unavailable (e.g., Windows 10 or unsupported graphics).
  - Snap Layout runtime limitation note: Custom HTML window caption buttons inside WebView2 cannot natively trigger the Windows 11 DWM Snap Layout flyout on hover without specialized low-level Win32 `WM_NCHITTEST` message interception on the top-level HWND.
- **No wasted sidebar**: Replaces previous 180px web-style sidebar with a top horizontal segmented control, freeing 100% of horizontal window width for settings content and two-column layout.
- **Scrollable body**: Smooth scrolling content viewport with purposeful hierarchy and 12px–16px desktop padding.
- **Persistent footer command bar (48px)**: Sticky bottom action bar housing live status feedback (`Settings apply immediately on save.`, spinner, or error alert) and primary actions (`Cancel` with `Esc` shortcut tag, and `Save changes`).

### 2. Design tokens & typography
- **Typography**: `"Plus Jakarta Sans", "Segoe UI", -apple-system, BlinkMacSystemFont, "Segoe UI Variable", "Segoe UI Emoji", system-ui, sans-serif`.
  - Font assets: Bundled local official variable WOFF2 fonts (`normal` and `italic`, 200..800 wght) licensed under SIL Open Font License (OFL 1.1) covering Latin, Latin Extended, Vietnamese (including compound diacritics `U+1EA0-1EF9`), and Cyrillic Extended subsets with `font-display: swap` and `tabular-nums` support.
  - Body text: 12px–13px (weight 400).
  - Form labels & buttons: 12px semi-bold / bold (weights 550 / 600).
  - Section headings: 12.5px bold (weight 600 / 650).
  - Page headings: 17px bold (weight 700).
- **Palette**:
  - Light mode: Slate/Zinc neutral desktop background (`#f1f5f9`), card surfaces (`#ffffff`), borders (`#e2e8f0` / `#cbd5e1`), Windows 11 accent blue (`#0067b8` primary, `#005fb8` hover, `#e0f2fe` subtle badge background). Text hierarchy: primary `#0f172a` (17.85:1 contrast), muted `#475569` (7.0:1 AAA contrast), subtle `#64748b` (4.76:1 AA contrast).
  - Dark mode: Windows dark slate (`#18181b` canvas, `#27272a` cards, `#202023` rows/inputs, `#3f3f46` borders, `#38bdf8` accent blue). Text hierarchy: primary `#f4f4f5` (13.5:1 AAA contrast), muted `#cbd5e1` (8.5:1 AAA contrast), subtle `#a1a1aa` (5.81:1 AA contrast). Direction codes use `#3f3f46` with `#f4f4f5` text.
  - Feedback tokens: Red (`#dc2626` / `#fef2f2` light; `#f87171` / `#450a0a` dark) for errors, amber (`#d97706` / `#fffbeb` light; `#fbbf24` / `#451a03` dark, 9.42:1 AA contrast) for duplicate warnings, green (`#059669` / `#ecfdf5` light; `#34d399` / `#064e3b` dark) for active status and save confirmations.
  - Status indicator tokens: When Settings is open, input hooks are intentionally suspended (`settings_suspended`), rendering a calm blue status pill (`#e0f2fe` bg / `#0067b8` text in light; `rgba(56, 189, 248, 0.15)` bg / `#38bdf8` text in dark) with label `"● Settings open"`. Preserves compact text on narrow viewports without collapsing to an unlabeled dot.
  - Button palette (pastel wedding skin): Main primary buttons use peach `#F7D6D0` to pink `#E2B4BD` gradient with dark text `#4A4A4A`; primary hover uses reversed gradient pink `#E2B4BD` to peach `#F7D6D0`, active uses solid peach `#F7D6D0` with subtle tactile depression (`transform: translateY(1px)`, `inset 0 1px 2px rgba(74, 74, 74, 0.15)`). Secondary/cancel buttons use blush `#FFF5F5` with visible pink border `#E2B4BD` and dark text `#4A4A4A`. All button surfaces and gradient points maintain WCAG AA compliant text contrast (>=4.5:1, range 4.85:1 to 8.28:1). Nav tab buttons and clickable wheel nodes share cohesive blush/pink states with non-color-only visible outlines (`2px solid #4A4A4A` in light, `#F7D6D0` in dark). Preserved across light/dark modes.
- **Control sizing**: Compact 30px control height with 4px border-radius, clean 1px borders, subtle focus rings (`2px solid #0067b8` on inputs, `2px solid #4A4A4A` on buttons). Range slider accented with `var(--win-accent)` and native Fluent focus ring.
- **Radial wheel tactile aesthetics & annular geometry**: Grounded with authentic circular game wheel aesthetics inspired by upstream `zhugez/Pingloom` (WheelEditor & overlay radial layout) while fully adapted to local design tokens. Slices are rendered as true 8-segment annular SVG wedges matching native North-clockwise geometry (`innerR=50`, `outerR=134`, `Ricon=92`) with a central cancel zone (`Cancel zone`). Slices feature subtle highlights and soft elevation (`box-shadow: 0 1px 3px rgba(0,0,0,0.05), inset 0 1px 0 rgba(255,255,255,0.7)` in light; `box-shadow: 0 2px 4px rgba(0,0,0,0.3), inset 0 1px 0 rgba(255,255,255,0.06)` in dark). The central cancel zone features an inset depression (`inset 0 2px 4px rgba(0,0,0,0.05)` light; `inset 0 2px 4px rgba(0,0,0,0.25)` dark).
- **Motion & transitions**: Controlled CSS transitions ensure smooth desktop responsiveness without unbounded timers or layout shifts:
  - Tab content enter: subtle bounded opacity/transform translation (140ms ease-out `translateY(2px)` to `0`).
  - Wheel slice node hover: snappy 100–120ms scale (1.08x) and blush glow (`box-shadow: 0 2px 6px rgba(226, 180, 189, 0.35)`).
  - Button depress: 80–100ms active depression (`scale(0.96)` for wheel nodes, `translateY(1px)` for command buttons).
  - Accessibility: `prefers-reduced-motion: reduce` disables panel entrances and decorative scales while retaining `translate(-50%, -50%)` for wheel positioning, visible state/focus feedback, and non-animated field reveal. Hidden Settings windows pause spinner animation without pausing the lifecycle heartbeat.
  - Feedback uses explicit visual transition properties (120ms), not layout or font interpolation. Repeated tab navigation leaves one active panel; focused wheel nodes retain press feedback.
  - Native motion: time-based wheel selection blending and a short opacity entrance do not change hit-testing or input timing. Ping/emote pop curves settle continuously at final size; all ping components fade together. Emote playback uses frame start times derived from WebP frame-end timestamps and begins when decoded frames arrive, without resetting on duplicate decode replies. Windows client-area animation preference suppresses decorative wheel/ping movement and uses static emotes. Native visual/runtime verification remains required.

### 3. Pages & controls
- **Triggers**:
  - Gesture activation copy: `Hold trigger + left mouse press, drag, and release to select a slice. Release in center to cancel.`
  - Ping and Emote trigger key dropdowns (including custom virtual keys).
  - Drag threshold numeric input (2–40 px) with unit tag and explanation.
  - Launch checkbox: `Enable gestures at launch`.
  - Global toggle shortcut: modifier pills (`Ctrl`, `Alt`, `Shift`, `Windows`), key selector, and live hotkey summary badge (e.g. `Ctrl + Alt + P`).
  - Pause over fullscreen games checkbox.
  - Instant trigger-tap actions for ping and emote, featuring real image thumbnail previews.
- **Tactical Pings**:
  - Two-column responsive layout (310px preview + controls).
  - Interactive radial wheel preview with 8 compass slices (N, NE, E, SE, S, SW, W, NW) using real bundled League PNG art.
  - Clicking any slice in the radial wheel highlights and focuses the corresponding slot selector.
  - Center cancel zone indicator (`CENTER / Cancel zone`).
  - Real-time duplicate detection tagging duplicate assignments in both the wheel preview and the slot list.
  - 8-row slot editor table with direction badges, real PNG thumbnails, and dropdown selectors.
  - Overlay display controls: Ping size (60–220 px) and Duration (1.0–8.0 s).
  - Audio controls: Selection ticks checkbox, Mute checkbox, Effects volume slider (0–100%) with tabular readout badge.
- **Emotes**:
  - Two-column responsive layout with 8-slice emote radial wheel preview.
  - 8-row emote slot editor with direction badges, real emote thumbnails, and dropdown selectors.
  - Emote overlay size (80–300 px) and bundled emote audio checkbox.

### 4. Responsiveness & accessibility
- **Viewports**:
  - Standard desktop (960x720 / 1000x760): Two-column layout with sticky radial preview on left and controls on right.
  - Native min-width (760x600) & Small desktop (720x600): Reflows into single column with scaled radial preview (240px × 240px), reducing preview card height from 380px to ~270px so direction slot rows and duplicate badges are immediately visible with minimal scrolling. Preserves compact status label (`"● Settings open"`, 10px). Zero horizontal overflow.
  - Display Zoom 200% (480x360 equivalent): Responsive header branding and non-shrinking action buttons ensure tab buttons (`Triggers`, `Tactical Pings`, `Emotes`) and footer actions (`Cancel [Esc]`, `Save changes`) remain 100% visible with zero text clipping.
- **Accessibility & keyboard interaction**:
  - Arrow keys cycle through navigation tabs (`ArrowLeft`, `ArrowRight`, `Home`, `End`).
  - Active tabpanels (`div[role="tabpanel"]`) have `tabindex="0"` for screen reader landmark focus.
  - `Escape` key cancels and closes Settings.
  - All form controls have associated labels and `data-field` attributes.
  - Canonical validation errors automatically navigate to the owning page and focus the invalid control.
  - High-contrast / forced-colors and reduced-motion media query support.

## Native ownership and security
Tauri owns the Settings UI loop; a dedicated pumped Win32 owner thread owns hooks, `APPLICATION`, gesture state, replay, overlay, tray and audio. The bridge uses bounded typed owned messages/replies, validation and serialized owner processing. Opening suspends/replays before showing; close/failure tears down the UI and resumes idempotently while retaining prior pause state. Save's canonical validation, atomic persistence and apply stay native. Capability is limited to the Settings window and event listening; local-origin navigation/CSP, with no remote navigation, arbitrary protocol/path, shell/filesystem plugin or server. Only embedded allowlisted PNG previews are exposed.

## Verification boundary & evidence
- **Automated browser loop**: Tested via headless Chrome with mock Tauri 2 IPC harness in `/tmp/test-runner/verify-ui.js` and durable repo runner `node ui/tests/verify-ui.cjs`:
  - Verified tab clickability for Triggers, Pings, and Emotes.
  - Verified real base64 PNG preview rendering for all 8 ping and emote slices.
  - Verified draft updates, duplicate warning badges, save payload consistency, validation error focus routing, and cancel bridge calls.
  - Zero browser console errors and zero runtime warning banners.
- **Quality audit evidence**: Comprehensive 14-point audit suite executed via `node /tmp/test-runner/audit-suite.js after`.
  - Screenshots in `.impeccable/review/quality-audit/after/`:
    - `01-triggers-960x720-light.png`
    - `02-pings-960x720-light.png`
    - `03-emotes-960x720-light.png`
    - `04-triggers-960x720-dark.png`
    - `05-pings-960x720-dark.png`
    - `06-emotes-960x720-dark.png`
    - `07-triggers-720x600-light.png`
    - `08-pings-720x600-light.png`
    - `09-emotes-720x600-light.png`
    - `10-pings-760x600-light.png`
    - `11-pings-zoom200.png`
    - `12-triggers-forcedcolors.png`
    - `13-emotes-reducedmotion.png`
    - `14-buttons-interactive.png`
  - Computed WCAG AA luminance metrics saved in `.impeccable/review/quality-audit/after/audit-results.json`:
    - Dark mode slot text: 13.5:1 (AAA)
    - Dark mode duplicate warning text: 9.42:1 (AAA)
    - Primary button gradient contrast: 4.85:1 to 6.53:1 (AA)
    - Cancel button blush contrast: 8.28:1 (AAA)
    - Light subtle hint text: 4.76:1 to 7.0:1 (AA/AAA)
    - Small viewport (720x600 & 760x600): 0px horizontal overflow
    - Zoom 200%: Zero button or tab text clipping
- **Windows GNU binary**: Rebuilt with embedded final `ui/dist` at `target/x86_64-pc-windows-gnu/debug/pingloom.exe`.
