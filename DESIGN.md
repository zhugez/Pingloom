# Pingloom desktop UI design

## Scope and ownership
One bundled Tauri 2 Settings WebView, built with Svelte 5, Tailwind/daisyUI and Lucide icons. Native Rust retains gestures, overlays, replay, animation/audio and tray. Windows is the original backend; Linux/X11 and macOS remain experimental. No remote UI assets or runtime font requests.

## Approved visual direction: Loom Day / Loom Night (V7)
Port of the user-approved Settings V7 canvas. Pingloom Blush `#e2b4bd` replaces Default Sapphire; the persisted `default` theme identifier remains compatible. Bricolage Grotesque supplies display headings, values and keycaps; Plus Jakarta Sans remains the body face. Both are bundled locally with Vietnamese subsets and OFL attribution.

- Day: ground `#fbf6f5`, white plates, blush secondary surfaces, ink `#3a3437`, muted `#6b6064`. Night: ground `#121013`, plates `#1a171b`, secondary surfaces `#221e23`, ink `#fff5f5`, muted `#bdb1b4`.
- Double bezels: 6px outer padding, 36px outer / 30px inner corners, fine boundaries and inset highlights. Pointer light follows the bezel, without changing layout.
- Low-opacity drifting background light, floating rounded header, separate pill navigation with a sliding selection, persistent Save island. No whole-panel transforms.
- Preserve the ribbon-woven P as a foreground-colored SVG mask on an opaque theme plate, with at least 4.5:1 contrast in every palette.
- Catppuccin Latte, Frappé, Macchiato and Mocha retain their canonical paper and Sapphire accent values. Theme preferences apply immediately and never save or modify the native settings draft.

## Layout
The shell is a viewport-height grid: titlebar, navigation, scrollable content, command footer. Navigation remains outside draggable regions. Native minimize, maximize/restore, double-click titlebar and generation-aware Close routing are preserved.

The Triggers page pairs a hold/drag/release demonstration with activation radio chips, a global shortcut keycap editor and tap actions. Platform-specific trigger choices and current custom virtual keys remain available. Emote choices disable the current Ping trigger; Rust remains the final validator. Threshold is a native range input (2–40px).

Pings and Emotes have a display heading, current configuration values, a live test wheel on the left and a selected-slice editor on the right. The editor offers previous/next slice controls, artwork, a visual library with assignment indicators and an expandable eight-row assignment table. Sound and custom imports follow the wheel. Range inputs preserve all existing bounds for size, duration and volume.

At compact desktop heights, the hero/demo and wheel scale down so the working surface begins earlier. At widths up to 760px, content becomes a single column. Header/footer compact further for 200% zoom; status remains accessible even when its visible label is hidden. Content scrolls while navigation and Save/Close remain available.

## Interaction and motion
- Click or keyboard-activate a wheel node to focus its editor; select a library tile to change the draft. Duplicate assignments have explicit Repeats feedback.
- Drag the local wheel beyond the configured threshold to select a slice. Release outside its central cancellation zone to preview; release in the centre or cancel the pointer to abort. This never invokes native input or saves settings.
- Test buttons use the same local preview. Size sliders briefly show a pixel-size preview. Preview images use still catalog artwork; native animation is unchanged. Preview audio is a labelled WebAudio synthesis, not the bundled clip; volume, mute, selection ticks and emote-sound preferences gate it. Custom emotes remain silent.
- The footer counts changed top-level settings fields. Reverting a field clears its dirty state; only a matching canonical Save reply marks it saved. Save and Close failures remain errors rather than success messages.
- Main easing: `cubic-bezier(.32,.72,0,1)`; key/switch/preview spring: `cubic-bezier(.34,1.56,.64,1)`. Finite child reveals never transform the page panel. Decorative pulse/drift/breathe loops are infinite. Selected wheel artwork resolves to integer 44px without stacked scaling.
- Reduced motion removes decorative motion, keeps the wheel centred and reveals fields without smooth scrolling. Hidden-window animations pause. Keyboard focus, native radio/range/checkbox semantics and forced-colors fallbacks remain supported.

## Native boundary and persistence
Drafts apply only on Save. Opening/closing Settings does not pause, cancel or resume native gestures. Native safety checks, permissions and manual Pause remain authoritative; status refreshes on the five-second heartbeat. The status indicator is display-only, not a pause button. No new IPC commands, dependencies or settings schema fields.

Imports use the existing bounded native `import_emote` command. Artwork is stored immediately; assignment remains a draft. PNG/JPG/GIF/WebP limits, local library limits, asynchronous errors and Save/import exclusion remain intact. Picker images stay still; the native overlay owns animated playback and audio.

Preserve the existing session/generation admission, owner-queue barriers, canonical Rust validation, atomic persistence and matching request IDs. Capability/CSP restrictions continue to allow only bundled Settings assets, not arbitrary paths or remote navigation.

## Native V7 Wheel
The actual native desktop Wheel now inherits V7 Night: separate dark hub, spaced wedges, compass labels/ticks, Blush selection arcs and subdued unselected art. This is not a WebView. Win32 and Unix renderers use the same `src/wheel.rs` surface, with geometry cached per open and only selection colors interpolated per frame. The bundled static label atlas uses Bricolage; rebuild it with `tools/render-wheel-labels.cjs` and Chromium. Rasterizing labels at open avoids resizing a full atlas every frame.

The native Wheel deliberately stays Night over arbitrary desktop backgrounds; the Settings theme selector remains Settings-only. The 86-DIP cancellation zone, slice mapping, input/replay and Ping/Emote effect artwork/motion remain unchanged. Windows retains its selected-action caption below the Wheel. See the visual-only amendment to ADR 0002.

`PINGLOOM_WHEEL_PREVIEW_DIR=/tmp/pingloom-v7-wheel cargo test --locked --release --bin pingloom native_v7_wheel_preview_at_display_scales -- --nocapture` renders real native Ping/Emote Wheels at 1x, 1.5x and 2x without OS input. `docs/media/pingloom-native-wheel-v7.png` shows that Unix raster output over a test background, not a physical desktop capture. Shared shape tests cover transparency, Blush arcs, slice mapping and the unchanged dead zone; a Windows canvas test checks the actual BGRA composition path on Windows CI.

## Verification boundary
`npm --prefix ui run check`, `npm --prefix ui test`, `npm --prefix ui run build` and `node ui/tests/verify-ui.cjs --refresh-readme` cover frontend checks, settings helpers and browser rendering with mocked desktop IPC. Browser checks include all palettes/brand contrast, HiDPI, compact windows, local drag/fire/cancel, size preview, dirty/revert, import, Save/error focus, chrome routing, interruption, hidden-window and reduced-motion behavior. The refresh command copies current captures into `docs/media/`, including the README preview. Chromium and puppeteer-core are test-harness prerequisites, not production dependencies.

The mechanical design detector flags the retained Plus Jakarta body face and approved spring easing. Those are reported, not suppressed or silently changed away from V7. No independent subagent design review was run in this session.

Browser screenshots are not physical desktop acceptance. WebView2 lifecycle, OS input/permissions, mixed DPI, native motion, tray and actual audio still need platform smoke checks. Native build/CI packaging does not certify those behaviors. The Windows owner-thread contract remains ADR 0001; proposed optimization work in ADR 0003 is separate and unimplemented.

Keep version **0.1.0** until the user explicitly authorizes a version increase.
