const REPO = 'https://github.com/zhugez/pingloom';
const VERSION = '0.1.0-opt.2';

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const version = env.PINGLOOM_VERSION || VERSION;
    const repo = env.GITHUB_REPO || REPO;
    const downloads = {
      windows: `${repo}/releases/download/v${version}/Pingloom_0.1.0_x64-setup.exe`,
      'windows-portable': `${repo}/releases/download/v${version}/pingloom.exe`,
      macos: `${repo}/releases/download/v${version}/pingloom-macos-universal.zip`,
    };
    const platform = url.pathname.slice('/download/'.length);
    if (url.pathname.startsWith('/download/') && Object.hasOwn(downloads, platform)) return Response.redirect(downloads[platform], 302);
    if (url.pathname === '/api/status') return Response.json({
      project: 'Pingloom', version, status: 'prerelease', edge_region: request.cf?.colo || 'EDGE',
      runtime: 'Cloudflare Workers', github: repo, deployment_url: 'https://app.pingloom.workers.dev',
      downloads: { windows_installer: downloads.windows, windows_portable: downloads['windows-portable'], macos_universal: downloads.macos },
    }, { headers: { 'Access-Control-Allow-Origin': '*' } });
    if (url.pathname !== '/') return env.ASSETS.fetch(request);
    return new Response(page(version), { headers: {
      'Content-Type': 'text/html; charset=utf-8',
      'Cache-Control': 'public, max-age=0, must-revalidate',
      'X-Content-Type-Options': 'nosniff',
      'X-Frame-Options': 'DENY',
      'Referrer-Policy': 'strict-origin-when-cross-origin',
    } });
  },
};

function page(version) {
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="color-scheme" content="dark">
  <title>Pingloom — Eight directions. One flick.</title>
  <meta name="description" content="Desktop pings and animated emotes, one gesture away. A native radial Wheel with locally bundled artwork and a familiar hold, drag, release gesture.">
  <link rel="icon" href="/images/mark.svg" type="image/svg+xml">
  <link rel="stylesheet" href="/style.css">
  <script src="/demo.js" type="module"></script>
</head>
<body>
  <a class="skip" href="#main">Skip to content</a>
  <header class="header wrap">
    <a class="brand" href="/" aria-label="Pingloom home"><span class="brand-mark" aria-hidden="true"></span>Pingloom</a>
    <nav aria-label="Main navigation"><a href="#playground">Try the Wheel</a><a href="#app">The app</a><a href="${REPO}">Source</a></nav>
    <a class="button primary" href="#download">Download</a>
  </header>

  <main id="main" class="wrap">
    <section class="hero" aria-labelledby="hero-title">
      <div class="hero-copy">
        <h1 id="hero-title">Eight directions.<br><span>One flick.</span></h1>
        <p>A little more expression for your desktop. Drop a ping. Send an emote. All with a familiar hold, drag, release.</p>
        <a class="button primary large" href="#download">Get Pingloom</a>
        <a class="text-link" href="#playground">Give the Wheel a spin <span aria-hidden="true">↓</span></a>
        <p class="availability">Windows · Experimental macOS · Free &amp; open source</p>
      </div>
      <figure class="hero-preview">
        <img src="/images/pingloom-native-wheel-v7.png" width="1004" height="502" alt="Pingloom’s native Ping and Emote Wheels with their actual bundled artwork" fetchpriority="high">
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
          <div class="effect-stage" id="effect-stage"><img id="selected-art" src="/assets/textures/pingwheel_retreatrender.png" width="160" height="160" alt="" hidden></div>
          <p class="direction" id="direction">N · North</p>
          <h3 id="selected-name">Danger</h3>
          <p id="selection-help">Artwork and sound from the desktop app. Select a slice to preview what it sends.</p>
          <button type="button" class="button secondary" id="replay" disabled>Preview again</button>
          <p id="announcement" class="sr-only" role="status" aria-live="polite"></p>
        </div>
      </div>
      <ol class="gesture-steps">
        <li><span>1</span><div><strong>Hold your trigger</strong><p>Choose a key or mouse button in Settings.</p></div></li>
        <li><span>2</span><div><strong>Pick a direction</strong><p>Drag toward a ping or emote on the Wheel.</p></div></li>
        <li><span>3</span><div><strong>Release to send</strong><p>Or return to the centre to cancel.</p></div></li>
      </ol>
    </section>

    <section id="app" class="app-section" aria-labelledby="app-title">
      <div class="app-copy"><h2 id="app-title">Make it<br>feel like you.</h2><p>Choose your trigger. Arrange your eight slices. Import your own emote images. Preview your changes before saving.</p><p>The Wheel and overlays run natively in Rust. Settings live in a bundled Tauri WebView, not on this website.</p><a class="text-link" href="${REPO}#readme">Explore the desktop app <span aria-hidden="true">↗</span></a></div>
      <figure class="app-preview bezel"><img src="/images/pingloom-sapphire-dark-preview.png" alt="Rendered Pingloom Settings in Loom Night: Tactical Pings, the eight-slice Wheel and selected Danger artwork" width="960" height="720" loading="lazy"><figcaption>Actual Settings UI render · Loom Night / Pingloom Blush</figcaption></figure>
    </section>

    <section id="download" class="downloads" aria-labelledby="download-title">
      <h2 id="download-title">A gesture away.</h2><p>Pingloom ${version} · Prerelease</p>
      <div class="download-options">
        <div><h3>Windows</h3><p>Windows 11 x64 · Requires Evergreen WebView2.</p><a class="button primary" href="/download/windows">Download installer</a><a class="text-link" href="/download/windows-portable">Portable .exe</a></div>
        <div><h3>macOS <small>Experimental</small></h3><p>Apple Silicon &amp; Intel. Requires native input permissions.</p><a class="button secondary" href="/download/macos">Download universal .zip</a></div>
      </div>
      <p class="download-note">Linux packages: coming soon. <a href="${REPO}/blob/main/docs/PLATFORMS.md">Platform requirements</a> · <a href="${REPO}/releases">Release notes</a></p>
    </section>
  </main>
  <footer class="wrap"><div class="footer-top"><a class="brand" href="/">Pingloom</a><span>Free &amp; open source. Code under MIT.</span><a href="${REPO}">View source</a></div><p>League ping and emote artwork © Riot Games, Inc. Not endorsed by Riot Games. <a href="/THIRD_PARTY_NOTICES.txt">Third-party notices</a>.</p></footer>
</body>
</html>`;
}
