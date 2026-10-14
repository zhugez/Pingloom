# Pingloom brand concept

Concept images generated with Gemini `gemini-3-pro-image`. High-contrast derivatives of the reconstructed SVG mark are used for application icons and the Settings header in the consolidated v0.1.0 prerelease.

- `pingloom-logo-concept.png`: logo exploration / wordmark direction.
- `pingloom-brandkit.png`: identity overview and conceptual desktop applications.
- `pingloom-mark.svg`: manually reconstructed, scalable mark based on the Gemini concept.
- `pingloom-mark-mono.svg`: cropped single-color mark with transparent crossing cuts; Settings uses it as a theme-foreground alpha mask.
- `pingloom-app-icon.svg`: dark, fixed high-contrast icon plate with white P/blush crossing, for desktop app icons.
- `pingloom-wordmark-light.png` / `pingloom-wordmark-dark.png`: transparent, lossless GitHub header lockups rendered with the real local Plus Jakarta Sans; README selects the contrasting variant with GitHub's `#gh-light-mode-only` / `#gh-dark-mode-only` fragments, so the GitHub theme—not merely OS preference—controls it.

## Direction

A ribbon-woven **P**, a circular counter, and a northeast opening: a small gesture releases
an expressive signal. The crossing ribbon refers to “loom”; no Riot artwork is used in
these new identity concepts. Originality/trademark clearance is not established by generation.

**Tagline:** Small gesture. Clear signal.

**Palette:** blush `#FFF5F5`, peach `#F7D6D0`, rose `#E2B4BD`, charcoal `#4A4A4A`.
**Typography:** the existing locally bundled Plus Jakarta Sans, with Vietnamese coverage.
Image-generated lettering is a visual reference, not a font asset or outlined wordmark.

The colored concept mark is for blush/peach surfaces, not an unqualified app/README
logo. Small Settings branding uses the palette foreground through the mono mask;
checks require at least 4.5:1 against the badge surface in all six palettes. Desktop
icons use the fixed dark plate; README uses the matching light/dark wordmark. Keep clear space of at least one ribbon width. Do not rotate, stretch,
add gradients, or replace the northeast opening with a generic sparkle/pin. The eight-slot
ring is a supporting gesture motif, not a second logo.

The board contains conceptual mockups, not screenshots or measured color/accessibility
proof. PNG generation may approximate colors/geometry; SVG values are authoritative for
the reconstructed mark. Small-size raster exports were generated for inspection, not
physical tray-icon acceptance. A final outlined wordmark is not included. Desktop icon
packages derived from `pingloom-app-icon.svg` are checked in under `icons/`. Run
`node ui/tests/render-brand.cjs` to regenerate icon formats and GitHub lockups.
Run `npm --prefix ui run preview:readme` to verify the current UI and refresh every
README screenshot. The latter requires Chromium/puppeteer-core like the browser harness.
