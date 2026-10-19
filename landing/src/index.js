import catalog from './catalog.json' with { type: 'json' };
import style from './style.js';

const REPO = 'https://github.com/zhugez/pingloom';
const SITE_URL = 'https://pingloom.zhugez.workers.dev';
const VERSION = '0.1.0';
const RELEASE_TAG = 'v0.1.0-opt.2';

const LLMS_TXT = `# Pingloom

> Native desktop pings and animated emotes for League of Legends, independent of the game process.

Pingloom is an open-source desktop application that places League of Legends tactical pings and animated emotes anywhere on your screen through an eight-slice radial gesture. Built with native Rust, it operates completely independent of the game process with zero memory injection or game client hooking.

## Desktop Controls
- **Activate & Drag**: Hold your keyboard trigger (default Alt for tactical pings, Ctrl for animated emotes) and drag outward with the left mouse button to open the radial wheel.
- **Send**: Release the left mouse button outside the centre to trigger the selected ping or emote at your gesture origin.
- **Cancel**: Release inside the central cancellation zone, press Escape, or click the right mouse button to abort without sending.

## Platforms & Requirements
- **Windows 11 x64**: Native Win32 hooks and layered overlays. Requires Evergreen WebView2 for Settings. Unsigned prerelease build.
- **macOS (Apple Silicon & Intel)**: Native Quartz/AppKit backend. Ad-hoc signed, unnotarized experimental build requiring Accessibility permission.
- **Linux**: Source backend retained; public binary packages paused.

## Project Links & Documentation
- [Homepage & Interactive Demo](https://pingloom.zhugez.workers.dev/): Browser preview of the 8-slice wheel and sound effects.
- [GitHub Repository](https://github.com/zhugez/pingloom): Source code, issues, and release downloads.
- [Gestures & Controls Guide](https://github.com/zhugez/pingloom#gestures--controls): Complete desktop gesture documentation.
- [Platform Requirements](https://github.com/zhugez/pingloom/blob/main/docs/PLATFORMS.md): Operating system requirements and security permissions.
- [Release Notes & Checksums](https://github.com/zhugez/pingloom/releases/tag/v0.1.0-opt.2): Verified CI build information and artifact hashes.
- [Third-Party Notices](https://pingloom.zhugez.workers.dev/THIRD_PARTY_NOTICES.txt): Attribution for Riot Games assets and open-source fonts.
`;

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const version = env.PINGLOOM_VERSION || VERSION;
    const releaseTag = env.PINGLOOM_RELEASE_TAG || RELEASE_TAG;
    const repo = env.GITHUB_REPO || REPO;
    const downloads = {
      windows: `${repo}/releases/download/${releaseTag}/Pingloom_0.1.0_x64-setup.exe`,
      'windows-portable': `${repo}/releases/download/${releaseTag}/pingloom.exe`,
      macos: `${repo}/releases/download/${releaseTag}/pingloom-macos-universal.zip`,
    };
    const platform = url.pathname.slice('/download/'.length);
    if (url.pathname.startsWith('/download/') && Object.hasOwn(downloads, platform)) return Response.redirect(downloads[platform], 302);
    if (url.pathname === '/api/status') return Response.json({
      project: 'Pingloom', version, release_tag: releaseTag, status: 'prerelease', edge_region: request.cf?.colo || 'EDGE',
      runtime: 'Cloudflare Workers', github: repo, deployment_url: SITE_URL,
      downloads: { windows_installer: downloads.windows, windows_portable: downloads['windows-portable'], macos_universal: downloads.macos },
    }, { headers: { 'Access-Control-Allow-Origin': '*' } });
    if (url.pathname === '/robots.txt') return new Response(
      `User-agent: *\nAllow: /\n\nSitemap: ${SITE_URL}/sitemap.xml\n`,
      {
        headers: {
          'Content-Type': 'text/plain; charset=utf-8',
          'Cache-Control': 'public, max-age=3600',
          'X-Content-Type-Options': 'nosniff',
        },
      }
    );
    if (url.pathname === '/sitemap.xml') return new Response(
      `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n  <url>\n    <loc>${SITE_URL}/</loc>\n  </url>\n</urlset>\n`,
      {
        headers: {
          'Content-Type': 'application/xml; charset=utf-8',
          'Cache-Control': 'public, max-age=3600',
          'X-Content-Type-Options': 'nosniff',
        },
      }
    );
    if (url.pathname === '/googled7ecf9fd7d293ff6.html') return new Response(
      'google-site-verification: googled7ecf9fd7d293ff6.html\n',
      {
        headers: {
          'Content-Type': 'text/html; charset=utf-8',
          'Cache-Control': 'public, max-age=3600',
          'X-Content-Type-Options': 'nosniff',
        },
      }
    );
    if (url.pathname === '/llms.txt') return new Response(
      LLMS_TXT,
      {
        headers: {
          'Content-Type': 'text/markdown; charset=utf-8',
          'Cache-Control': 'public, max-age=3600',
          'X-Content-Type-Options': 'nosniff',
        },
      }
    );
    if (url.pathname !== '/') {
      const assetResponse = await env.ASSETS.fetch(request);
      if (assetResponse.status !== 200) return assetResponse;
      const response = new Response(assetResponse.body, assetResponse);
      if (
        url.pathname.startsWith('/fonts/') ||
        url.pathname.startsWith('/assets/') ||
        url.pathname.startsWith('/images/') ||
        url.pathname.endsWith('.css') ||
        url.pathname.endsWith('.js')
      ) {
        response.headers.set('Cache-Control', 'public, max-age=31536000, immutable');
      } else if (url.pathname.endsWith('.json')) {
        response.headers.set('Cache-Control', 'public, max-age=86400, stale-while-revalidate=604800');
      }
      return response;
    }
    return new Response(page(version, releaseTag, repo, SITE_URL), { headers: {
      'Content-Type': 'text/html; charset=utf-8',
      'Cache-Control': 'public, max-age=0, must-revalidate',
      'X-Content-Type-Options': 'nosniff',
      'X-Frame-Options': 'DENY',
      'Referrer-Policy': 'strict-origin-when-cross-origin',
    } });
  },
};

function page(version, releaseTag, repo, siteUrl = SITE_URL) {
  const title = 'Desktop Pings & Animated Emotes | Pingloom';
  const description = 'Place League of Legends tactical pings and animated emotes on your desktop with an eight-slice radial gesture. Native Rust app, independent of the game process.';
  const ogImage = `${siteUrl}/images/pingloom-native-wheel-v7.png`;
  const schema = {
    '@context': 'https://schema.org',
    '@type': 'SoftwareApplication',
    name: 'Pingloom',
    applicationCategory: 'UtilitiesApplication',
    operatingSystem: 'Windows 11, macOS',
    softwareVersion: version,
    url: `${siteUrl}/`,
    sameAs: repo,
    license: 'https://opensource.org/licenses/MIT',
    downloadUrl: `${siteUrl}/#download`,
    image: ogImage,
    screenshot: ogImage,
    description,
    offers: {
      '@type': 'Offer',
      price: '0',
      priceCurrency: 'USD',
    },
    author: {
      '@type': 'Person',
      name: 'zhugez',
      url: 'https://github.com/zhugez',
    },
    featureList: [
      'Eight-slice radial selection wheel for tactical pings and animated emotes',
      'Independent of game process with zero memory injection',
      'Native Rust input hooks and transparent desktop overlays',
      'Custom animated WebP and GIF emote import support',
      'Theme-synced appearance matching the desktop Settings interface',
    ],
  };
  const jsonLd = JSON.stringify(schema, null, 2).replace(/</g, '\\u003c');
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="color-scheme" content="dark">
  <title>${title}</title>
  <meta name="description" content="${description}">
  <meta name="google-site-verification" content="googled7ecf9fd7d293ff6">
  <link rel="canonical" href="${siteUrl}/">
  <link rel="preload" href="/fonts/plus-jakarta-sans-latin-wght-normal.woff2" as="font" type="font/woff2" crossorigin>
  <link rel="preload" href="/fonts/bricolage-grotesque-latin-wght-normal.woff2" as="font" type="font/woff2" crossorigin>
  <link rel="preload" href="/images/pingloom-native-wheel-v7-640.webp" imagesrcset="/images/pingloom-native-wheel-v7-640.webp 640w, /images/pingloom-native-wheel-v7.webp 1004w" imagesizes="(max-width: 700px) calc(100vw - 36px), 630px" as="image" type="image/webp" fetchpriority="high">
  <link rel="modulepreload" href="/demo.js">
  <script id="catalog-data" type="application/json">${JSON.stringify(catalog)}</script>
  <meta property="og:type" content="website">
  <meta property="og:site_name" content="Pingloom">
  <meta property="og:title" content="${title}">
  <meta property="og:description" content="${description}">
  <meta property="og:url" content="${siteUrl}/">
  <meta property="og:image" content="${ogImage}">
  <meta property="og:image:width" content="1004">
  <meta property="og:image:height" content="502">
  <meta property="og:image:alt" content="Pingloom native eight-slice Ping and Emote Wheels with bundled artwork">
  <meta name="twitter:card" content="summary_large_image">
  <meta name="twitter:title" content="${title}">
  <meta name="twitter:description" content="${description}">
  <meta name="twitter:image" content="${ogImage}">
  <meta name="twitter:image:alt" content="Pingloom native eight-slice Ping and Emote Wheels with bundled artwork">
  <link rel="icon" href="/images/mark.svg" type="image/svg+xml">
  <style>${style}</style>
  <script type="application/ld+json">${jsonLd}</script>
</head>
<body>
  <a class="skip" href="#main">Skip to content</a>
  <header class="header wrap">
    <a class="brand" href="/" aria-label="Pingloom home"><span class="brand-mark" aria-hidden="true"></span>Pingloom</a>
    <nav aria-label="Main navigation"><a href="#playground">Try the Wheel</a><a href="#app">The app</a><a href="${repo}">Source</a></nav>
    <a class="button primary" href="#download">Download</a>
  </header>

  <main id="main" class="wrap">
    <section class="hero" aria-labelledby="hero-title">
      <div class="hero-copy">
        <h1 id="hero-title"><span class="sr-only">Pingloom — </span>Eight directions.<br><span>One flick.</span></h1>
        <p>Bring League of Legends tactical pings and animated emotes to your desktop with an eight-slice radial gesture. Runs natively in Rust, independent of the game process.</p>
        <a class="button primary large" href="#download">Get Pingloom</a>
        <a class="text-link" href="#playground">Give the Wheel a spin <span aria-hidden="true">↓</span></a>
        <p class="availability">Windows · Experimental macOS · Free &amp; open source</p>
      </div>
      <figure class="hero-preview">
        <picture>
          <source srcset="/images/pingloom-native-wheel-v7-640.webp 640w, /images/pingloom-native-wheel-v7.webp 1004w" sizes="(max-width: 700px) calc(100vw - 36px), 630px" type="image/webp">
          <img src="/images/pingloom-native-wheel-v7.png" width="1004" height="502" alt="Pingloom’s native Ping and Emote Wheels with their actual bundled artwork" fetchpriority="high" decoding="async">
        </picture>
        <figcaption>Two Wheels. Your own way to say it.<small>Native renderer preview, not a desktop capture.</small></figcaption>
      </figure>
    </section>

    <section id="playground" class="playground" aria-labelledby="play-title">
      <div class="section-top">
        <div><h2 id="play-title">Feel the flick.</h2><p>Same artwork. Same eight directions. Try it right here.</p></div>
        <div class="mode-switch" role="group" aria-label="Wheel type">
          <button type="button" data-mode="pings" aria-pressed="true">Tactical Pings</button>
          <button type="button" data-mode="emotes" aria-pressed="false">Emotes</button>
        </div>
      </div>
      <div class="demo-layout">
        <div class="bezel"><div class="plate wheel-plate">
          <div class="demo-toolbar"><span>Browser preview</span><button type="button" id="sound" aria-pressed="false">Sound off</button></div>
          <div id="wheel" class="wheel" role="group" aria-label="Eight-slice Wheel. Drag outward and release, or select a slice with the keyboard." aria-describedby="wheel-help">
            <svg viewBox="0 0 460 460" aria-hidden="true">
              <circle cx="230" cy="230" r="224" class="ticks"/>
              <g id="wedges"></g>
              <circle cx="230" cy="230" r="86" class="hub"/>
              <text x="230" y="224" class="hub-label">RELEASE</text><text x="230" y="244" class="hub-sub">to cancel</text>
            </svg>
            <div id="nodes"></div>
          </div>
          <p id="wheel-help">Hold, drag out, release. Return to the centre to cancel.<br>Or tab to a slice and press Enter.</p>
          <p id="demo-error" role="alert" hidden>Could not load the preview. Refresh the page to try again.</p>
        </div></div>
        <div class="selection">
          <div class="effect-stage" id="effect-stage"><picture id="selected-picture"><source id="selected-source" srcset="/assets/textures/pingwheel_retreatrender.webp" type="image/webp"><img id="selected-art" src="/assets/textures/pingwheel_retreatrender.png" width="160" height="160" alt="" loading="lazy" decoding="async" hidden></picture></div>
          <p class="direction" id="direction">N · North</p>
          <h3 id="selected-name">Danger</h3>
          <p id="selection-help">Artwork and sound from the desktop app. Select a slice to preview what it sends.</p>
          <button type="button" class="button secondary" id="replay" disabled>Preview again</button>
          <p id="announcement" class="sr-only" role="status" aria-live="polite"></p>
        </div>
      </div>
      <ol class="gesture-steps">
        <li><span>1</span><div><strong>Hold your trigger key</strong><p>Hold your keyboard trigger (default Alt for pings, Ctrl for emotes).</p></div></li>
        <li><span>2</span><div><strong>Drag with the left mouse button</strong><p>Keep the trigger held, press the left mouse button and drag toward a slice.</p></div></li>
        <li><span>3</span><div><strong>Release left mouse to send</strong><p>Release the left mouse button outside the centre to send. Release inside the central cancellation zone, press Escape, or right-click to cancel.</p></div></li>
      </ol>
      <p class="gesture-note">Desktop controls use a held keyboard trigger and left mouse drag; mouse-button triggers drag with that same button. Pings and emotes appear at the initial mouse-press origin on your desktop. The browser preview above lets you test slices directly without holding keys. See the <a href="${repo}#gestures--controls">usage guide</a> and <a href="${repo}/blob/main/docs/PLATFORMS.md">platform requirements</a>.</p>
    </section>

    <section id="app" class="app-section" aria-labelledby="app-title">
      <div class="app-copy"><h2 id="app-title">Make it<br>feel like you.</h2><p>Choose your trigger. Arrange your eight slices. Import your own emote images. Preview your changes before saving.</p><p>The Wheel and overlays run natively in Rust. Settings live in a bundled Tauri WebView, not on this website.</p><a class="text-link" href="${repo}#readme">Explore the desktop app <span aria-hidden="true">↗</span></a></div>
      <figure class="app-preview bezel">
        <picture>
          <source srcset="/images/pingloom-sapphire-dark-preview-720.webp 720w, /images/pingloom-sapphire-dark-preview.webp 960w" sizes="(max-width: 700px) calc(100vw - 36px), 720px" type="image/webp">
          <img src="/images/pingloom-sapphire-dark-preview.png" alt="Rendered Pingloom Settings in Loom Night: Tactical Pings, the eight-slice Wheel and selected Danger artwork" width="960" height="720" loading="lazy" decoding="async">
        </picture>
        <figcaption>Actual Settings UI render · Loom Night / Pingloom Blush</figcaption>
      </figure>
    </section>

    <section id="download" class="downloads" aria-labelledby="download-title">
      <h2 id="download-title">A gesture away.</h2><p>Pingloom v${version} · Prerelease</p>
      <div class="download-options">
        <div class="download-card">
          <h3>Windows</h3>
          <p class="download-requirements">Windows 11 x64 · Requires Evergreen WebView2.</p>
          <div class="download-cta">
            <a class="button primary" href="/download/windows">Download installer</a>
            <a class="text-link" href="/download/windows-portable">Portable .exe</a>
          </div>
          <p class="download-warning">Unsigned prerelease build. Windows SmartScreen may show an unrecognized app prompt on first run.</p>
          <p class="download-verify"><a href="${repo}/releases/download/${releaseTag}/BUILD_INFO-windows.txt">Windows checksum &amp; build info</a> · <a href="${repo}/blob/main/docs/PLATFORMS.md#windows">Windows platform guide</a></p>
        </div>
        <div class="download-card">
          <h3>macOS <small>Experimental</small></h3>
          <p class="download-requirements">Apple Silicon &amp; Intel universal binary.</p>
          <div class="download-cta">
            <a class="button secondary" href="/download/macos">Download universal .zip</a>
          </div>
          <p class="download-warning">Ad-hoc signed, unnotarized experimental build. Gatekeeper may ask for approval in Privacy &amp; Security. Gestures need Accessibility and, if prompted, Input Monitoring.</p>
          <p class="download-verify"><a href="${repo}/releases/download/${releaseTag}/BUILD_INFO-macos.txt">macOS checksum &amp; build info</a> · <a href="${repo}/blob/main/docs/PLATFORMS.md#macos">macOS platform guide</a></p>
        </div>
      </div>
      <p class="download-note">Linux packages: coming soon. <a href="${repo}/blob/main/docs/PLATFORMS.md">Platform requirements</a> · <a href="${repo}/releases/tag/${releaseTag}">Release notes</a></p>
    </section>
  </main>
  <footer class="wrap"><div class="footer-top"><a class="brand" href="/">Pingloom</a><span>Free &amp; open source. Code under MIT.</span><a href="${repo}">View source</a></div><p>League ping and emote artwork © Riot Games, Inc. Not endorsed by Riot Games. <a href="/THIRD_PARTY_NOTICES.txt">Third-party notices</a>.</p></footer>
  <script src="/demo.js" type="module"></script>
</body>
</html>`;
}
