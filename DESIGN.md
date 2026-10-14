# Pingloom desktop UI design

## Scope and ownership
One bundled Tauri 2 Settings WebView using Svelte 5, Tailwind and daisyUI with Lucide icons. Native Rust retains gestures, overlays, replay, audio and tray. Windows is the original backend; Linux/X11 and macOS remain experimental. No production server or remote UI assets.

## lolPing interaction/motion refresh (shipped in v0.1.0)
Reference: upstream lolPing `f24a51e5a11c2b31dab72d89a571cc4025ad3c98`,
`WheelEditor.tsx`, `overlay/wheel.ts`, `overlay/overlay.css`, and `emoteFx.ts`.
This refresh adapts its visual catalog interaction and motion semantics, not its
Electron runtime, multiplayer, custom imports, or complete feature set.

- Choose a wheel slice, then an illustrated catalog tile; keyboard activation is
  supported. Full dropdown assignments remain in an expandable section, and
  validation opens that section before focusing its field.
- Dark radial preview and native wheel surfaces make the wheel distinct from the
  Settings shell; theme accent highlights preserve Pingloom identity.
- Short panel entrances, selected-icon emphasis and keyed assignment feedback;
  reduced motion removes decorative movement.
- Native Windows/Unix ping art shares diagonal two-layer convergence, stable
  positioning during pop, entrance alpha and gentle bob. Ring edges use analytical
  coverage rather than hard/double-stamped pixels.
- Bundled animated WebP plays its baked motion without a second pop/fade envelope;
  still fallbacks and reduced-motion rendering retain their bounded fade.
- Native artwork uses a shared premultiplied-alpha cubic resampler. This avoids
  dark fringes from filtering straight RGBA on Unix and nearest-neighbor stair
  steps on Windows; same-size images bypass resampling. Regression tests cover
  transparent-black and hidden-color edges.
- Settings avoids whole-panel translation and stacked hover/selection scales.
  Selected artwork resolves to an integer 44px size. Review captures wait for
  fonts and finite animations, assert settled panel state, and include a 2x
  device-scale PNG. Native motion review uses lossless APNG rather than relying
  on GIF's quantized palette. These checks do not certify native mixed-DPI behavior.
- Gesture timing, hit testing, schema, permissions and version are unchanged.

Browser evidence: `node ui/tests/verify-ui.cjs` (mock IPC). Native raster evidence:
`PINGLOOM_MOTION_PREVIEW_DIR=/tmp/pingloom-native-motion cargo test --bin pingloom native_motion_frames_are_visible_then_expire`.
The latter exports deterministic frames from the Unix rasterizer, not an OS screen
recording; Windows visual acceptance still requires hardware. The three-platform
v0.1.0 release now includes these changes from build commit `f073e22`.

## Approved visual direction
Warm, quiet desktop utility: flat property groups rather than nested cards. Keep Plus Jakarta Sans locally bundled with Vietnamese, Latin/extended and Cyrillic-extended coverage and OFL attribution.

- Warm white canvas `#FFFDFC`, blush surfaces `#FFF5F5`, charcoal text `#4A4A4A`.
- Default Sapphire `#85C1DC` is the Settings accent, with dark ink on fills and contrasting control boundaries.
- Settings-only theme controls offer Default Sapphire and the official Catppuccin Latte, Frappé, Macchiato and Mocha palettes; this renderer preference is immediate and independent of native Settings Save.
- DaisyUI default themes are disabled. Checkbox/range primitives share explicit shell tokens, avoiding stray blue/purple defaults.
- Theme palettes provide their own dark surfaces and Sapphire accents; forced-colors and reduced-motion rules remain supported.
- The ribbon-woven P is preserved but must remain legible across themes: Settings uses a cropped monochrome SVG alpha mask colored with `--ink`, not hard-coded charcoal/rose. Desktop icons use a fixed dark plate, white P/blush crossing and light perimeter; GitHub gets transparent light/dark wordmark PNGs selected through `<picture>`. Review small native sizes on light/dark backgrounds and require ≥4.5:1 Settings logo contrast in all six palettes.

## Layout and typography
Separate 44px titlebar, 52px tab navigation, scrollable body and persistent 62px command footer. Window controls preserve native minimize/maximize and generation-aware close. Navigation is outside draggable titlebar regions.

Page headings are 24px; form text is 13px, controls 36px tall. Descriptions use secondary text, not extra enclosing panels. Main navigation remains Triggers, Tactical Pings and Emotes.

Pings/Emotes place an interactive eight-slice wheel on the left and a flat directional editor on the right. Display settings sit below the wheel; audio follows the workspace. Real bundled artwork sits directly on the wedges without rectangular tiles. Names remain available through accessible buttons and the corresponding editor. Emotes has an accessible PNG/JPG/GIF/WebP import input above its picker, explicit size/library limits, asynchronous status/error feedback and disabled import/save while importing. Imported artwork joins the picker and wheel/tap dropdowns; assignment remains a draft until Save. Picker thumbnails stay still (including GIF/WebP); animation runs in the native overlay for its normal three-second lifetime and uses a first-frame fallback for reduced motion. The center denotes cancellation. Selection, duplicate warnings and error states are retained.

At widths up to 760px the workspace becomes a single column; narrow windows use smaller chrome without hiding action buttons. Content scrolls, while navigation and Save/Close stay available.

## Behavior and accessibility
- Preserve all existing trigger, wheel assignment, size, duration and audio controls.
- Arrow/Home/End keys navigate tabs; Escape cancels; invalid fields receive focus on the correct page.
- Wheel selection focuses its editor; duplicate assignments receive explicit warnings.
- Opening/closing Settings does not pause, cancel or resume gestures. Explicit pause and native safety failures remain authoritative. A bounded owner-queue barrier prevents late accepted saves from crossing into a reopened session. Saving safely cancels an in-progress gesture before applying settings; drafts are not live. The header refreshes active/paused status on the five-second heartbeat.
- Save remains canonical Rust validation followed by atomic persistence and native apply.
- Capability and command guards restrict access to the bundled Settings document and label, including Tauri's empty root-document path. No arbitrary filesystem paths or remote navigation.
- Reduced motion preserves wheel positioning and focus feedback while suppressing decorative motion. Hidden-window spinner behavior remains bounded.

## Verification boundary
`npm --prefix ui run check`, `npm --prefix ui test`, `npm --prefix ui run build` and `node ui/tests/verify-ui.cjs` cover frontend checks, settings logic, rendering, navigation, draft/save/cancel, validation focus, native-chrome command routing and reduced-motion behavior.

`npm --prefix ui run preview:readme` builds the current UI, runs all browser checks including six-palette logo contrast, and copies the verified pages/theme captures into `docs/media/`. Refresh these whenever Settings or branding changes, before publication; README imagery is part of the feature, not a separate follow-up. Chromium/puppeteer-core are browser harness prerequisites. The release/native icon provenance remains separate from mocked UI evidence.

Browser screenshots in `.impeccable/review/desktop-redesign/` render the real frontend with mocked desktop IPC. They are not physical desktop acceptance. Actual input, OS materials, permissions, DPI, tray rendering and audio still need platform smoke testing. Native compilation and CI packaging do not certify those behaviors.

The user approved this visual direction. Keep the consolidated prerelease at **0.1.0** until an explicit version increase is authorized.
