# Media Assets & Provenance

This directory stores visual documentation and UI preview artifacts for Pingloom.

## Assets

| File | Type | Provenance & Description |
|------|------|--------------------------|
| `pingloom-banner.svg` | Vector Banner | Authored simple vector SVG for GitHub repository presentation. Features an 8-slice radial wheel motif with pastel tokens (`#FFF5F5`, `#F7D6D0`, `#E2B4BD`, `#4A4A4A`) and system font fallbacks (`Plus Jakarta Sans`, `Segoe UI`, `system-ui`). Contains no external fonts, no CDN dependencies, and no scripts. |
| `pingloom-pings-preview.png` | UI Preview (PNG) | Rendered browser preview of the Pingloom Tactical Pings Settings tab, generated via the headless test harness (`ui/tests/verify-ui.cjs`) against `ui/dist`. Note: this is a rendered UI preview, not a native Windows 11 DWM capture. |
| `pingloom-emotes-preview.png` | UI Preview (PNG) | Rendered browser preview of the Pingloom Emotes Settings tab, generated via the headless test harness (`ui/tests/verify-ui.cjs`) against `ui/dist`. Note: this is a rendered UI preview, not a native Windows 11 DWM capture. |
| `banner.png` | Legacy Banner | Preserved legacy banner from initial repository state. |

## Fonts and Licensing

- **UI Typography**: Bundled local WOFF2 variable fonts for **Plus Jakarta Sans** (weights 200..800, normal and italic), licensed under the [SIL Open Font License 1.1](https://openfontlicense.org/).
- **League Assets**: Ping icons and emote graphics are © Riot Games, Inc. and are not covered by the MIT license. Not endorsed by Riot Games.
