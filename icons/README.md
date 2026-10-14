# Desktop icons

Source: `docs/media/brand/pingloom-app-icon.svg`, retaining the ribbon-woven P.
The fixed dark plate `#202330`, white P and blush crossing are independent of the
Settings palette; the light boundary stays visible on dark desktops. Outer corners
remain transparent. Do not reuse the low-contrast concept-board imagery as icons.

Regenerate the 1024px RGBA PNG, Windows ICO, macOS ICNS and transparent GitHub
light/dark wordmarks with:

```sh
node ui/tests/render-brand.cjs
```

Requires the browser harness Chromium/puppeteer-core setup and installed UI dependencies.
The script renders vector truth/local Plus Jakarta Sans, then uses the installed
Tauri CLI for desktop formats; no generated Android/iOS icon scaffolding is retained.

Tauri embeds the Windows ICO at resource ID **32512**. Native icon loading uses the
executable module, not a generic system application icon. Settings uses the separate
cropped monochrome SVG as a CSS alpha mask colored with its theme foreground.

Review 16/24/32/48/64/256px sizes on both light and dark backgrounds. Raster checks
and embedded-resource comparison are not physical desktop/tray acceptance.
