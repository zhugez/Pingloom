# Pingloom landing

One fixed **Loom Night / Pingloom Blush** theme, matching the desktop app.

```sh
npm ci
npm test
npm run dev
npm run deploy
```

`build.mjs` reads `../src/model.rs` for the default Ping and Emote Wheels, then copies their actual PNG, animated WebP and OGG assets into generated `public/`. It also copies bundled fonts, brand artwork, third-party notices and existing app renders from `docs/media/`. No duplicate artwork catalogue, external image URLs or synthetic audio.

The browser demo supports pointer dragging, centre cancellation, keyboard slice activation and optional real asset audio. Reduced-motion users get still artwork. This is a browser approximation of the gesture, not native desktop input or a latency benchmark.

Images are labelled by provenance: the native Wheel image is a native renderer raster preview; the Settings screenshot is a browser-rendered app UI, not a physical desktop capture. League artwork belongs to Riot Games and is not MIT-licensed; see the linked third-party notices.

## Deployment

Worker `pingloom` serves `/`, `/api/status`, `/robots.txt`, `/sitemap.xml`, `/download/windows`, `/download/windows-portable`, `/download/macos` and locally hosted static assets via the `ASSETS` binding in `wrangler.toml`. Worker configuration in `wrangler.toml` separates the visible application version (`PINGLOOM_VERSION = "0.1.0"`) from the distribution release tag (`PINGLOOM_RELEASE_TAG = "v0.1.0-opt.3"`).

## Search metadata and crawl endpoints

- **Metadata**: Fixed self-canonical `https://pingloom.zhugez.workers.dev/`, Google Search Console verification meta tag and `/googled7ecf9fd7d293ff6.html` route, OpenGraph and Twitter `summary_large_image` tags referencing the native wheel preview asset (`1004×502`), font/image preload hints in `<head>`, and Schema.org `SoftwareApplication` JSON-LD reflecting truthful free pricing, repository `sameAs` link, MIT license, and supported OS platforms without fabricated reviews or certifications.
- **Crawl endpoints**: `/robots.txt` specifies normal allow with the absolute sitemap location; `/sitemap.xml` lists only the canonical homepage URL; `/llms.txt` provides structured markdown documentation for answer engines and AI search crawlers. Endpoints are served directly by the Worker before static asset fallback.
- **Validation limitations**: Automated tests validate routing, headers, schema structure, checksum link targets and content offline. Cloudflare managed zones may return edge-level robots directives; local Worker responses do not guarantee or prove remote edge policy overrides.

`.github/workflows/deploy-landing.yml` runs tests and a dry-run on pull requests; pushes to `main` and manual runs on `main` deploy to https://pingloom.zhugez.workers.dev/. Changes to the source catalogue, assets, fonts or app renders also trigger this workflow.

GitHub Actions needs `CLOUDFLARE_API_TOKEN` with Workers edit permission and `CLOUDFLARE_ACCOUNT_ID` for the account owning the `zhugez.workers.dev` subdomain. A one-off MCP deployment does not configure those secrets.

`public/` is generated and ignored; always run the build before invoking Wrangler directly.

Optional browser check (requires Playwright Core and Chromium, with the dev server running):

```sh
PLAYWRIGHT_MODULE=/path/to/playwright-core/index.mjs CHROME_PATH=/path/to/chrome node tests/browser.mjs
```

Checks desktop and 390/320px layouts, both asset catalogues, keyboard activation, drag/cancel, sound controls and reduced motion. Captures go to `.impeccable/review/landing/` at the repository root.
