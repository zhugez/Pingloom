/**
 * Cloudflare Worker hosting the Pingloom Landing Page.
 * Edge-rendered with sub-millisecond response time.
 * Target deployment: https://app.pingloom.workers.dev/
 */

export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);
    const version = env?.PINGLOOM_VERSION || '0.1.0-opt.2';
    const repo = env?.GITHUB_REPO || 'https://github.com/zhugez/pingloom';
    const edgeRegion = request.cf?.colo || 'EDGE';

    // Platform download redirects
    if (url.pathname === '/download/windows') {
      return Response.redirect(`${repo}/releases/download/v${version}/Pingloom_0.1.0_x64-setup.exe`, 302);
    }
    if (url.pathname === '/download/windows-portable') {
      return Response.redirect(`${repo}/releases/download/v${version}/pingloom.exe`, 302);
    }
    if (url.pathname === '/download/macos') {
      return Response.redirect(`${repo}/releases/download/v${version}/pingloom-macos-universal.zip`, 302);
    }

    // Health / metadata API
    if (url.pathname === '/api/status') {
      return new Response(JSON.stringify({
        project: 'Pingloom',
        version,
        status: 'prerelease',
        edge_region: edgeRegion,
        runtime: 'Cloudflare Workers',
        github: repo,
        deployment_url: 'https://app.pingloom.workers.dev',
        downloads: {
          windows_installer: `${repo}/releases/download/v${version}/Pingloom_0.1.0_x64-setup.exe`,
          windows_portable: `${repo}/releases/download/v${version}/pingloom.exe`,
          macos_universal: `${repo}/releases/download/v${version}/pingloom-macos-universal.zip`,
        }
      }, null, 2), {
        headers: {
          'Content-Type': 'application/json',
          'Access-Control-Allow-Origin': '*',
        }
      });
    }

    // Serve landing page HTML
    return new Response(renderLandingHtml(version, repo, edgeRegion), {
      headers: {
        'Content-Type': 'text/html; charset=utf-8',
        'Cache-Control': 'public, max-age=3600, s-maxage=86400',
        'X-Content-Type-Options': 'nosniff',
        'X-Frame-Options': 'DENY',
        'Referrer-Policy': 'strict-origin-when-cross-origin',
      },
    });
  },
};

function renderLandingHtml(version, repo, edgeRegion) {
  return `<!DOCTYPE html>
<html lang="en" data-theme-mode="dark" data-theme-palette="default-dark">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Pingloom — Native Radial Gestures for Desktop</title>
  <meta name="description" content="Tactile 8-slice radial gestures for pings and emotes on any screen. Zero game hooks, sub-millisecond Rust input loop, and bundled Tauri 2 settings." />
  <meta property="og:title" content="Pingloom — Native Radial Gestures for Desktop" />
  <meta property="og:description" content="Tactile pings and League emotes for desktop. Zero game hooks, sub-millisecond input hooks, and bundled Tauri 2 settings." />
  <meta property="og:type" content="website" />
  <meta property="og:url" content="https://app.pingloom.workers.dev/" />
  <meta name="twitter:card" content="summary_large_image" />

  <link rel="icon" type="image/svg+xml" href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 480 480'><defs><mask id='m'><rect width='480' height='480' fill='white'/><path d='M80 348 220 244' fill='none' stroke='black' stroke-width='72'/></mask></defs><g fill='%23e2b4bd' mask='url(%23m)'><rect x='100' y='204' width='64' height='244' rx='8'/><path d='M322.56 115.14A116 116 0 1 0 336.86 129.44' fill='none' stroke='%23e2b4bd' stroke-width='64'/></g><path d='M80 348 220 244' fill='none' stroke='%23ffffff' stroke-width='56'/></svg>">

  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=Bricolage+Grotesque:opsz,wght@12..96,600;12..96,700;12..96,800&family=JetBrains+Mono:wght@400;600;700&family=Plus+Jakarta+Sans:wght@400;500;600;700;800&display=swap" rel="stylesheet">

  <style>
    /* Pingloom V7 Design Tokens (Loom Day / Loom Night & Catppuccin) */
    :root {
      font-family: 'Plus Jakarta Sans', system-ui, sans-serif;
      font-variant-numeric: tabular-nums;
      line-height: 1.5;
      color-scheme: dark;

      --display: 'Bricolage Grotesque', 'Plus Jakarta Sans', sans-serif;
      --mono: 'JetBrains Mono', monospace;
      --ease: cubic-bezier(.32,.72,0,1);
      --spring: cubic-bezier(.34,1.56,.64,1);

      /* Default: Loom Night */
      --paper: #121013;
      --surface: #221e23;
      --core: #1a171b;
      --core2: #221e23;
      --ink: #fff5f5;
      --muted: #bdb1b4;
      --faint: #958a8d;
      --line: rgba(255, 245, 245, 0.08);
      --input-line: #958a8d;
      --accent: #e2b4bd;
      --accent-ink: #2a2026;
      --accent-text: #e2b4bd;
      --strong: #e2b4bd;
      --focus: #e2b4bd;
      --key-edge: #0b090c;
      --wedge-in: #1b171c;
      --wedge-out: #28232a;
      --bar: rgba(26, 23, 27, 0.82);
      --island: rgba(34, 30, 35, 0.88);
      --shadow: rgba(0, 0, 0, 0.7);
      --hi: rgba(255, 255, 255, 0.07);
      --shell: rgba(255, 245, 245, 0.04);
      --shell-line: rgba(255, 245, 245, 0.08);
      --soft: rgba(226, 180, 189, 0.16);
      --accent-line: rgba(226, 180, 189, 0.45);
      --glow: rgba(226, 180, 189, 0.35);
      --spot: rgba(226, 180, 189, 0.45);
      --spot-core: rgba(226, 180, 189, 0.08);
      --top-glow: rgba(226, 180, 189, 0.18);
      --aurora-a: rgba(226, 180, 189, 0.14);
      --aurora-b: rgba(247, 214, 208, 0.08);
      --tick: rgba(255, 245, 245, 0.15);
      --tick-major: #e2b4bd;
      --dots: rgba(255, 245, 245, 0.07);
      --success: #9ed9b5;
      --warning: #f3c089;
      --error: #ff9c9e;
    }

    /* Loom Day Palette */
    :root[data-theme-palette="default-light"] {
      color-scheme: light;
      --paper: #fbf6f5;
      --surface: #fdf3f2;
      --core: #ffffff;
      --core2: #fdf3f2;
      --ink: #3a3437;
      --muted: #6b6064;
      --faint: #786d71;
      --line: #e9dfe0;
      --input-line: #88767d;
      --accent: #e2b4bd;
      --accent-ink: #2e2428;
      --accent-text: #715a5f;
      --strong: #8c7075;
      --focus: #715a5f;
      --key-edge: #e4d3d5;
      --wedge-in: #ffffff;
      --wedge-out: #f7e8e9;
      --bar: rgba(255, 252, 251, 0.85);
      --island: rgba(255, 252, 251, 0.92);
      --shadow: rgba(74, 40, 50, 0.14);
      --hi: rgba(255, 255, 255, 0.95);
      --shell: rgba(74, 58, 62, 0.04);
      --shell-line: rgba(74, 58, 62, 0.08);
      --soft: rgba(226, 180, 189, 0.22);
      --accent-line: rgba(226, 180, 189, 0.65);
      --glow: rgba(226, 180, 189, 0.38);
      --spot: rgba(226, 180, 189, 0.55);
      --spot-core: rgba(226, 180, 189, 0.1);
      --top-glow: rgba(226, 180, 189, 0.32);
      --aurora-a: rgba(226, 180, 189, 0.28);
      --aurora-b: rgba(247, 214, 208, 0.5);
      --tick: rgba(74, 58, 62, 0.18);
      --tick-major: #8c7075;
      --dots: rgba(74, 58, 62, 0.1);
      --success: #2f7a55;
      --warning: #8a4f0e;
      --error: #b52e3b;
    }

    /* Catppuccin Latte */
    :root[data-theme-palette="latte"] {
      color-scheme: light;
      --paper: #eff1f5;
      --surface: #e6e9ef;
      --core: #ffffff;
      --core2: #e6e9ef;
      --ink: #4c4f69;
      --muted: #5c5f77;
      --faint: #6c6f85;
      --line: #ccd0da;
      --input-line: #5c5f77;
      --accent: #209fb5;
      --accent-ink: #11111b;
      --accent-text: #209fb5;
      --strong: #246b7a;
      --focus: #4c4f69;
      --key-edge: #ccd0da;
      --bar: rgba(239, 241, 245, 0.88);
      --shell: rgba(76, 79, 105, 0.05);
      --shell-line: rgba(76, 79, 105, 0.12);
      --soft: rgba(32, 159, 181, 0.16);
      --accent-line: rgba(32, 159, 181, 0.45);
      --glow: rgba(32, 159, 181, 0.3);
      --spot: rgba(32, 159, 181, 0.4);
      --spot-core: rgba(32, 159, 181, 0.06);
      --tick: rgba(92, 95, 119, 0.25);
      --tick-major: #209fb5;
      --dots: rgba(92, 95, 119, 0.12);
    }

    /* Catppuccin Frappé */
    :root[data-theme-palette="frappe"] {
      color-scheme: dark;
      --paper: #303446;
      --surface: #292c3c;
      --core: #232634;
      --core2: #292c3c;
      --ink: #c6d0f5;
      --muted: #b5bfe2;
      --faint: #838ba7;
      --line: #414559;
      --accent: #85c1dc;
      --accent-ink: #11111b;
      --accent-text: #85c1dc;
      --strong: #85c1dc;
      --key-edge: #232634;
      --bar: rgba(41, 44, 60, 0.85);
      --shell: rgba(198, 208, 245, 0.05);
      --shell-line: rgba(198, 208, 245, 0.1);
      --soft: rgba(133, 193, 220, 0.18);
      --accent-line: rgba(133, 193, 220, 0.4);
      --glow: rgba(133, 193, 220, 0.28);
      --spot: rgba(133, 193, 220, 0.4);
      --spot-core: rgba(133, 193, 220, 0.06);
      --tick: rgba(181, 191, 226, 0.2);
      --tick-major: #85c1dc;
      --dots: rgba(181, 191, 226, 0.1);
    }

    /* Catppuccin Mocha */
    :root[data-theme-palette="mocha"] {
      color-scheme: dark;
      --paper: #1e1e2e;
      --surface: #181825;
      --core: #11111b;
      --core2: #181825;
      --ink: #cdd6f4;
      --muted: #bac2de;
      --faint: #9399b2;
      --line: #313244;
      --accent: #74c7ec;
      --accent-ink: #11111b;
      --accent-text: #74c7ec;
      --strong: #74c7ec;
      --key-edge: #11111b;
      --bar: rgba(24, 24, 37, 0.85);
      --shell: rgba(205, 214, 244, 0.05);
      --shell-line: rgba(205, 214, 244, 0.1);
      --soft: rgba(116, 199, 236, 0.18);
      --accent-line: rgba(116, 199, 236, 0.4);
      --glow: rgba(116, 199, 236, 0.28);
      --spot: rgba(116, 199, 236, 0.4);
      --spot-core: rgba(116, 199, 236, 0.06);
      --tick: rgba(186, 194, 222, 0.2);
      --tick-major: #74c7ec;
      --dots: rgba(186, 194, 222, 0.1);
    }

    * { box-sizing: border-box; margin: 0; padding: 0; }
    html { scroll-behavior: smooth; }

    body {
      background-color: var(--paper);
      color: var(--ink);
      font-family: 'Plus Jakarta Sans', system-ui, sans-serif;
      min-height: 100vh;
      overflow-x: hidden;
      position: relative;
      transition: background-color 0.3s var(--ease), color 0.3s var(--ease);
    }

    ::selection { background: var(--accent); color: var(--accent-ink); }

    /* Atmosphere Lighting */
    .atmosphere {
      position: fixed;
      inset: 0;
      overflow: hidden;
      pointer-events: none;
      z-index: 0;
    }
    .top-glow {
      position: absolute;
      width: 1240px;
      height: 760px;
      left: 50%;
      top: -380px;
      transform: translateX(-50%);
      background: radial-gradient(closest-side, var(--top-glow), transparent);
    }
    .aurora {
      position: absolute;
      border-radius: 50%;
      pointer-events: none;
    }
    .aurora-a {
      width: 780px;
      height: 780px;
      top: -240px;
      left: -200px;
      background: radial-gradient(closest-side, var(--aurora-a), transparent);
      animation: drift-a 16s var(--ease) infinite alternate;
    }
    .aurora-b {
      width: 860px;
      height: 860px;
      top: 400px;
      right: -260px;
      background: radial-gradient(closest-side, var(--aurora-b), transparent);
      animation: drift-b 19s var(--ease) infinite alternate;
    }
    @keyframes drift-a {
      0% { transform: translate(0, 0) scale(1); }
      100% { transform: translate(80px, 60px) scale(1.1); }
    }
    @keyframes drift-b {
      0% { transform: translate(0, 0) scale(1); }
      100% { transform: translate(-70px, -50px) scale(1.08); }
    }

    .container {
      max-width: 1240px;
      margin: 0 auto;
      padding: 0 24px;
      position: relative;
      z-index: 1;
    }

    /* Double Bezel System (App V7 Signature) */
    .bezel {
      min-width: 0;
      padding: 6px;
      border: 1px solid var(--shell-line);
      border-radius: 36px;
      background: radial-gradient(380px circle at var(--mx, -999px) var(--my, -999px), var(--spot), transparent 70%), var(--shell);
      transition: border-color 0.25s var(--ease);
    }
    .bezel:hover {
      border-color: var(--accent-line);
    }
    .plate {
      position: relative;
      height: 100%;
      min-width: 0;
      padding: 32px;
      border-radius: 30px;
      background: radial-gradient(560px circle at var(--mx, -999px) var(--my, -999px), var(--spot-core), transparent 65%), var(--core);
      box-shadow: inset 0 1px 0 var(--hi), 0 20px 48px -24px var(--shadow);
    }

    /* Floating Header */
    header {
      position: sticky;
      top: 14px;
      z-index: 100;
      margin: 14px auto 0;
      max-width: 1240px;
      padding: 8px 12px 8px 16px;
      border: 1px solid var(--shell-line);
      border-radius: 28px;
      background: var(--bar);
      backdrop-filter: blur(20px) saturate(160%);
      box-shadow: 0 24px 60px -36px var(--shadow), inset 0 1px 0 var(--hi);
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 16px;
    }
    .brand {
      display: flex;
      align-items: center;
      gap: 12px;
      text-decoration: none;
      color: var(--ink);
    }
    .brand-badge {
      display: grid;
      place-items: center;
      width: 40px;
      height: 40px;
      border-radius: 14px;
      border: 1px solid var(--line);
      background: var(--core2);
      box-shadow: inset 0 1px 0 var(--hi);
    }
    .brand-svg {
      width: 26px;
      height: 26px;
      display: block;
    }
    .brand-text {
      display: flex;
      flex-direction: column;
      line-height: 1.15;
    }
    .brand-name {
      font-family: var(--display);
      font-weight: 800;
      font-size: 18px;
      letter-spacing: -0.02em;
    }
    .brand-sub {
      font-size: 11px;
      color: var(--muted);
      font-weight: 500;
    }

    /* Status Pill */
    .status-pill {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      padding: 6px 14px;
      border: 1px solid var(--line);
      border-radius: 999px;
      background: var(--shell);
      font-size: 11.5px;
      font-weight: 700;
      color: var(--ink);
      white-space: nowrap;
    }
    .status-dot {
      position: relative;
      width: 7px;
      height: 7px;
      border-radius: 50%;
      background: var(--success);
    }
    .status-dot::before {
      content: '';
      position: absolute;
      inset: 0;
      border-radius: inherit;
      background: inherit;
      animation: live-pulse 1.8s var(--ease) infinite;
    }
    @keyframes live-pulse {
      0% { transform: scale(1); opacity: 0.9; }
      100% { transform: scale(2.8); opacity: 0; }
    }

    /* Nav Tabs */
    .nav-tabs {
      display: flex;
      align-items: center;
      gap: 4px;
      background: var(--shell);
      padding: 4px;
      border-radius: 999px;
      border: 1px solid var(--shell-line);
    }
    .nav-link {
      padding: 6px 14px;
      border-radius: 999px;
      color: var(--muted);
      text-decoration: none;
      font-size: 12.5px;
      font-weight: 700;
      transition: all 0.2s var(--ease);
    }
    .nav-link:hover {
      color: var(--ink);
      background: var(--core2);
    }

    /* Theme Controls in Header */
    .header-actions {
      display: flex;
      align-items: center;
      gap: 10px;
    }
    .theme-select {
      height: 38px;
      padding: 0 10px;
      border: 1px solid var(--line);
      border-radius: 12px;
      background: var(--core2);
      color: var(--ink);
      font-size: 12px;
      font-weight: 600;
      cursor: pointer;
      outline: none;
    }
    .theme-select:focus-visible {
      border-color: var(--accent);
    }

    /* Buttons */
    .btn {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      gap: 8px;
      height: 40px;
      padding: 0 18px;
      border-radius: 999px;
      font-size: 13px;
      font-weight: 750;
      text-decoration: none;
      cursor: pointer;
      transition: all 0.22s var(--ease);
      border: 1px solid transparent;
      white-space: nowrap;
    }
    .btn-primary {
      background: var(--accent);
      color: var(--accent-ink);
      box-shadow: 0 6px 20px -6px var(--glow);
    }
    .btn-primary:hover {
      transform: translateY(-1px);
      filter: brightness(1.06);
      box-shadow: 0 10px 28px -6px var(--glow);
    }
    .btn-secondary {
      background: var(--core2);
      color: var(--ink);
      border-color: var(--line);
    }
    .btn-secondary:hover {
      background: var(--soft);
      border-color: var(--accent-line);
      color: var(--accent-text);
    }

    /* Eyebrow Chip */
    .eyebrow {
      display: inline-table;
      margin-bottom: 20px;
      padding: 6px 14px;
      border-radius: 999px;
      background: var(--soft);
      color: var(--accent-text);
      font: 800 11px/1.4 'Plus Jakarta Sans', sans-serif;
      letter-spacing: 0.18em;
      text-transform: uppercase;
      border: 1px solid var(--accent-line);
    }

    /* Hero Section */
    .hero {
      padding: 80px 0 50px;
      text-align: center;
      display: flex;
      flex-direction: column;
      align-items: center;
    }
    .hero h1 {
      font-family: var(--display);
      font-size: clamp(42px, 6.4vw, 84px);
      font-weight: 800;
      line-height: 1.02;
      letter-spacing: -0.035em;
      max-width: 960px;
      margin-bottom: 22px;
    }
    .hero h1 kbd {
      display: inline-flex;
      align-items: center;
      height: 1.05em;
      padding: 0 0.28em;
      vertical-align: 0.05em;
      border: 1px solid var(--line);
      border-radius: 0.22em;
      background: var(--core2);
      color: var(--accent-text);
      box-shadow: 0 0.08em 0 var(--key-edge);
      font-size: 0.72em;
    }
    .hero-lead {
      font-size: clamp(16px, 2vw, 19px);
      color: var(--muted);
      max-width: 700px;
      line-height: 1.65;
      margin-bottom: 36px;
    }
    .hero-ctas {
      display: flex;
      gap: 14px;
      flex-wrap: wrap;
      justify-content: center;
      margin-bottom: 54px;
    }

    /* 3-Step Gesture Walkthrough Banner */
    .gesture-walkthrough {
      width: 100%;
      max-width: 900px;
      margin: 0 auto 64px;
    }
    .walkthrough-grid {
      display: grid;
      grid-template-columns: repeat(3, 1fr);
      gap: 20px;
      align-items: center;
      text-align: center;
    }
    .walk-step {
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 14px;
      padding: 20px 14px;
      border-radius: 20px;
      background: var(--shell);
      border: 1px solid var(--shell-line);
    }
    .walk-step-num {
      font-size: 11px;
      font-weight: 800;
      letter-spacing: 0.14em;
      text-transform: uppercase;
      color: var(--accent-text);
    }
    .demo-keycap {
      display: grid;
      place-items: center;
      width: 74px;
      height: 74px;
      border: 1px solid var(--line);
      border-radius: 20px;
      background: var(--core);
      box-shadow: 0 5px 0 var(--key-edge);
      color: var(--accent-text);
      font: 800 24px var(--display);
    }
    .walk-step-label {
      font-size: 13px;
      font-weight: 700;
      color: var(--ink);
    }
    .walk-step-sub {
      font-size: 11.5px;
      color: var(--muted);
    }

    /* Interactive Simulator Stage */
    .simulator-section {
      margin-bottom: 96px;
    }
    .sim-header {
      display: flex;
      justify-content: space-between;
      align-items: flex-end;
      flex-wrap: wrap;
      gap: 20px;
      margin-bottom: 24px;
    }
    .sim-title-group h2 {
      font-family: var(--display);
      font-size: 32px;
      font-weight: 800;
      letter-spacing: -0.025em;
    }
    .sim-title-group p {
      color: var(--muted);
      font-size: 14px;
      margin-top: 4px;
    }
    .sim-mode-toggle {
      display: flex;
      align-items: center;
      gap: 4px;
      padding: 4px;
      border-radius: 999px;
      background: var(--core2);
      border: 1px solid var(--line);
    }
    .sim-mode-btn {
      padding: 6px 16px;
      border-radius: 999px;
      border: none;
      background: transparent;
      color: var(--muted);
      font-size: 12px;
      font-weight: 700;
      cursor: pointer;
      transition: all 0.2s var(--ease);
    }
    .sim-mode-btn.active {
      background: var(--accent);
      color: var(--accent-ink);
    }

    .sim-workspace {
      display: grid;
      grid-template-columns: minmax(0, 1.35fr) minmax(0, 1fr);
      gap: 24px;
      align-items: stretch;
    }

    /* Radial Wheel Canvas */
    .wheel-arena-card {
      position: relative;
      overflow: hidden;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      min-height: 520px;
      user-select: none;
      touch-action: none;
      cursor: grab;
    }
    .wheel-arena-card:active {
      cursor: grabbing;
    }
    .dot-grid {
      position: absolute;
      inset: 0;
      background-image: radial-gradient(var(--dots) 1px, transparent 1.5px);
      background-size: 22px 22px;
      mask-image: radial-gradient(circle at 50% 50%, #000 30%, transparent 75%);
      pointer-events: none;
    }
    .wheel-container {
      position: relative;
      width: 440px;
      height: 440px;
      max-width: 100%;
    }
    #wheelSvg {
      width: 100%;
      height: 100%;
      overflow: visible;
    }
    .radial-wedge {
      fill: var(--wedge-in);
      stroke: var(--line);
      stroke-width: 1.2;
      transition: fill 0.16s var(--ease), stroke 0.16s var(--ease);
      cursor: pointer;
    }
    .radial-wedge.active {
      fill: var(--soft);
      stroke: var(--accent-line);
      stroke-width: 2;
    }
    .hub-circle {
      fill: var(--core);
      stroke: var(--line);
      stroke-width: 1.5;
      transition: fill 0.2s var(--ease), stroke 0.2s var(--ease);
    }
    .hub-circle.cancelling {
      fill: var(--soft);
      stroke: var(--accent);
    }
    .highlight-arc {
      fill: none;
      stroke: var(--accent);
      stroke-width: 3.5;
      stroke-linecap: round;
      pointer-events: none;
      transition: opacity 0.2s var(--ease);
    }
    .drag-indicator-line {
      stroke: var(--accent);
      stroke-width: 2.5;
      stroke-linecap: round;
      pointer-events: none;
    }
    .wheel-audio-toggle {
      position: absolute;
      top: 16px;
      right: 16px;
      display: inline-flex;
      align-items: center;
      gap: 6px;
      padding: 6px 12px;
      border-radius: 999px;
      background: var(--core2);
      border: 1px solid var(--line);
      font-size: 11.5px;
      font-weight: 700;
      color: var(--muted);
      cursor: pointer;
      z-index: 10;
    }
    .wheel-audio-toggle.enabled {
      color: var(--accent-text);
      border-color: var(--accent-line);
      background: var(--soft);
    }
    .wheel-footer-hint {
      position: absolute;
      bottom: 14px;
      font-size: 11.5px;
      color: var(--faint);
      font-family: var(--mono);
      pointer-events: none;
      text-align: center;
    }

    /* Slice Inspector Side Card */
    .slice-inspector {
      display: flex;
      flex-direction: column;
      justify-content: space-between;
      gap: 24px;
    }
    .inspector-header {
      display: flex;
      align-items: center;
      gap: 14px;
      padding-bottom: 20px;
      border-bottom: 1px solid var(--line);
    }
    .active-badge-preview {
      display: grid;
      place-items: center;
      width: 68px;
      height: 68px;
      border-radius: 20px;
      background: var(--soft);
      border: 1px solid var(--accent-line);
      box-shadow: 0 8px 24px -6px var(--glow);
    }
    .inspector-title {
      font-family: var(--display);
      font-size: 24px;
      font-weight: 800;
      letter-spacing: -0.02em;
    }
    .inspector-sub {
      font-size: 12.5px;
      color: var(--muted);
      margin-top: 2px;
    }
    .slice-properties {
      display: grid;
      grid-template-columns: repeat(2, 1fr);
      gap: 14px;
    }
    .prop-card {
      padding: 14px;
      border-radius: 16px;
      background: var(--core2);
      border: 1px solid var(--line);
    }
    .prop-label {
      font-size: 10.5px;
      font-weight: 800;
      letter-spacing: 0.14em;
      text-transform: uppercase;
      color: var(--faint);
      margin-bottom: 4px;
    }
    .prop-val {
      font-family: var(--mono);
      font-size: 15px;
      font-weight: 700;
      color: var(--ink);
    }
    .sim-explanation {
      font-size: 13.5px;
      color: var(--muted);
      line-height: 1.65;
      background: var(--shell);
      padding: 16px;
      border-radius: 18px;
      border: 1px solid var(--shell-line);
    }

    /* Ripple Effect */
    .ping-ripple {
      position: absolute;
      pointer-events: none;
      border-radius: 50%;
      transform: translate(-50%, -50%);
      animation: ripple-burst 0.75s var(--ease) forwards;
      z-index: 20;
    }
    @keyframes ripple-burst {
      0% { width: 0; height: 0; opacity: 1; border: 3px solid var(--ripple-color); }
      100% { width: 220px; height: 220px; opacity: 0; border: 1px solid var(--ripple-color); }
    }

    /* Bento Grid (Architecture) */
    .bento-section {
      margin-bottom: 96px;
    }
    .section-eyebrow {
      text-align: center;
      margin-bottom: 12px;
    }
    .section-title {
      font-family: var(--display);
      font-size: clamp(30px, 4vw, 44px);
      font-weight: 800;
      letter-spacing: -0.03em;
      text-align: center;
      margin-bottom: 48px;
    }
    .bento-grid {
      display: grid;
      grid-template-columns: repeat(3, 1fr);
      gap: 24px;
    }
    .bento-card-wide {
      grid-column: span 2;
    }
    .bento-icon-box {
      width: 48px;
      height: 48px;
      border-radius: 15px;
      background: var(--soft);
      border: 1px solid var(--accent-line);
      display: grid;
      place-items: center;
      color: var(--accent-text);
      margin-bottom: 20px;
    }
    .bento-card h3 {
      font-family: var(--display);
      font-size: 21px;
      font-weight: 800;
      letter-spacing: -0.02em;
      margin-bottom: 10px;
    }
    .bento-card p {
      color: var(--muted);
      font-size: 14px;
      line-height: 1.65;
    }

    /* Telemetry Table */
    .telemetry-section {
      margin-bottom: 96px;
    }
    .telemetry-grid {
      display: grid;
      grid-template-columns: repeat(4, 1fr);
      gap: 20px;
      margin-top: 32px;
    }
    .metric-card {
      padding: 24px;
      border-radius: 20px;
      background: var(--core2);
      border: 1px solid var(--line);
      text-align: center;
    }
    .metric-value {
      font-family: var(--mono);
      font-size: 36px;
      font-weight: 800;
      color: var(--accent-text);
      line-height: 1.1;
      margin-bottom: 8px;
    }
    .metric-label {
      font-size: 11.5px;
      font-weight: 700;
      letter-spacing: 0.1em;
      text-transform: uppercase;
      color: var(--faint);
    }
    .metric-desc {
      font-size: 12px;
      color: var(--muted);
      margin-top: 6px;
    }

    /* Downloads Matrix */
    .downloads-section {
      margin-bottom: 96px;
    }
    .downloads-grid {
      display: grid;
      grid-template-columns: repeat(2, 1fr);
      gap: 24px;
      margin-top: 36px;
    }
    .dl-platform-header {
      display: flex;
      align-items: center;
      gap: 16px;
      margin-bottom: 18px;
    }
    .dl-platform-icon {
      width: 52px;
      height: 52px;
      border-radius: 16px;
      background: var(--soft);
      border: 1px solid var(--accent-line);
      display: grid;
      place-items: center;
      color: var(--accent-text);
    }
    .dl-platform-title h4 {
      font-family: var(--display);
      font-size: 22px;
      font-weight: 800;
      letter-spacing: -0.02em;
    }
    .dl-platform-title span {
      font-size: 12.5px;
      color: var(--muted);
    }
    .dl-buttons-row {
      display: flex;
      gap: 12px;
      flex-wrap: wrap;
      margin-top: 24px;
    }

    /* Footer */
    footer {
      border-top: 1px solid var(--shell-line);
      padding: 48px 0;
      display: flex;
      justify-content: space-between;
      align-items: center;
      flex-wrap: wrap;
      gap: 20px;
      font-size: 13px;
      color: var(--muted);
    }
    footer a {
      color: var(--ink);
      text-decoration: none;
      transition: color 0.15s;
    }
    footer a:hover {
      color: var(--accent-text);
    }
    .edge-badge {
      font-family: var(--mono);
      font-size: 11px;
      padding: 3px 8px;
      border-radius: 6px;
      background: var(--shell);
      border: 1px solid var(--shell-line);
    }

    /* Responsive Breakpoints */
    @media (max-width: 960px) {
      .bento-grid { grid-template-columns: 1fr; }
      .bento-card-wide { grid-column: span 1; }
      .sim-workspace { grid-template-columns: 1fr; }
      .telemetry-grid { grid-template-columns: repeat(2, 1fr); }
      .downloads-grid { grid-template-columns: 1fr; }
      .walkthrough-grid { grid-template-columns: 1fr; }
    }
    @media (max-width: 640px) {
      header { padding: 8px 12px; }
      .nav-tabs { display: none; }
      .wheel-container { width: 340px; height: 340px; }
      .telemetry-grid { grid-template-columns: 1fr; }
    }
  </style>
</head>

<body>
  <!-- Atmosphere Background Glows -->
  <div class="atmosphere">
    <div class="top-glow"></div>
    <div class="aurora aurora-a"></div>
    <div class="aurora aurora-b"></div>
  </div>

  <!-- Sticky Glass Header Bar -->
  <header>
    <a href="/" class="brand">
      <div class="brand-badge">
        <svg class="brand-svg" viewBox="0 0 480 480" fill="none">
          <defs>
            <mask id="brandWeave" maskUnits="userSpaceOnUse" x="0" y="0" width="480" height="480">
              <rect width="480" height="480" fill="white"/>
              <path d="M80 348 220 244" fill="none" stroke="black" stroke-width="72"/>
            </mask>
          </defs>
          <g fill="currentColor" mask="url(#brandWeave)">
            <rect x="100" y="204" width="64" height="244" rx="8"/>
            <path d="M322.56 115.14A116 116 0 1 0 336.86 129.44" fill="none" stroke="currentColor" stroke-width="64"/>
          </g>
          <path d="M80 348 220 244" fill="none" stroke="var(--accent)" stroke-width="56"/>
        </svg>
      </div>
      <div class="brand-text">
        <span class="brand-name">Pingloom</span>
        <span class="brand-sub">Native Radial Gestures</span>
      </div>
    </a>

    <div class="status-pill">
      <span class="status-dot"></span>
      <span>0.02ms p95 • Edge Cached</span>
    </div>

    <nav class="nav-tabs">
      <a href="#simulator" class="nav-link">Simulator</a>
      <a href="#architecture" class="nav-link">Architecture</a>
      <a href="#telemetry" class="nav-link">Telemetry</a>
      <a href="#downloads" class="nav-link">Downloads</a>
    </nav>

    <div class="header-actions">
      <select class="theme-select" id="themeSelect" aria-label="Select Palette">
        <option value="default-dark" selected>Loom Night (Default)</option>
        <option value="default-light">Loom Day (Blush)</option>
        <option value="latte">Catppuccin Latte</option>
        <option value="frappe">Catppuccin Frappé</option>
        <option value="mocha">Catppuccin Mocha</option>
      </select>
      <a href="#downloads" class="btn btn-primary">Get v${version}</a>
    </div>
  </header>

  <main class="container">
    <!-- Hero Section -->
    <section class="hero">
      <div class="eyebrow">
        ZERO GAME HOOKS • SUB-MILLISECOND LATENCY • NATIVE RUST ENGINE
      </div>
      <h1>
        Desktop Pings & Emotes.<br>
        Radial Gestures on <kbd>Any Screen</kbd>.
      </h1>
      <p class="hero-lead">
        Summon League's tactile 8-slice radial wheel anywhere on your desktop. Hold your trigger key across Discord, browser windows, code editors or full-screen games to dispatch instant tactical pings and animated emotes.
      </p>
      <div class="hero-ctas">
        <a href="#downloads" class="btn btn-primary" style="height:46px; padding:0 24px; font-size:14.5px;">
          <svg width="18" height="18" fill="none" stroke="currentColor" stroke-width="2.2" viewBox="0 0 24 24"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
          Download for Windows &amp; Mac
        </a>
        <a href="${repo}" target="_blank" rel="noopener noreferrer" class="btn btn-secondary" style="height:46px; padding:0 20px; font-size:14.5px;">
          <svg width="18" height="18" fill="currentColor" viewBox="0 0 24 24"><path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z"/></svg>
          View Source (MIT)
        </a>
      </div>

      <!-- 3-Step Gesture Walkthrough -->
      <div class="gesture-walkthrough">
        <div class="bezel">
          <div class="plate" style="padding: 24px;">
            <div class="walkthrough-grid">
              <div class="walk-step">
                <span class="walk-step-num">Step 01 • Trigger</span>
                <div class="demo-keycap">G</div>
                <div class="walk-step-label">Hold Trigger Key</div>
                <div class="walk-step-sub">Global key or mouse button</div>
              </div>
              <div class="walk-step">
                <span class="walk-step-num">Step 02 • Aim</span>
                <svg width="74" height="74" viewBox="0 0 100 100" fill="none">
                  <circle cx="50" cy="50" r="44" stroke="var(--line)" stroke-dasharray="4 4"/>
                  <line x1="50" y1="50" x2="80" y2="20" stroke="var(--accent)" stroke-width="3" stroke-linecap="round"/>
                  <circle cx="80" cy="20" r="6" fill="var(--accent)"/>
                  <circle cx="50" cy="50" r="18" fill="var(--core2)" stroke="var(--line)"/>
                </svg>
                <div class="walk-step-label">Drag Cursor Outward</div>
                <div class="walk-step-sub">Past the 24px dead-zone</div>
              </div>
              <div class="walk-step">
                <span class="walk-step-num">Step 03 • Cast</span>
                <div style="width:74px; height:74px; display:grid; place-items:center;">
                  <div style="width:48px; height:48px; border-radius:50%; background:var(--soft); border:2px solid var(--accent); display:grid; place-items:center; color:var(--accent-text);">
                    <svg width="24" height="24" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24"><polygon points="12 2 15 8 22 9 17 14 18 21 12 17 6 21 7 14 2 9 9 8 12 2"/></svg>
                  </div>
                </div>
                <div class="walk-step-label">Release to Dispatch</div>
                <div class="walk-step-sub">Instant visual ping &amp; sound</div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>

    <!-- Interactive Simulator Section -->
    <section class="simulator-section" id="simulator">
      <div class="sim-header">
        <div class="sim-title-group">
          <div class="eyebrow" style="margin-bottom:8px;">Live Tactile Playground</div>
          <h2>Interactive Radial Wheel Simulator</h2>
          <p>Drag outward with your mouse or finger to preview slices. Release to cast a ping with authentic WebAudio synthesis.</p>
        </div>
        <div style="display:flex; align-items:center; gap:12px;">
          <div class="sim-mode-toggle">
            <button class="sim-mode-btn active" id="modePings">Tactical Pings</button>
            <button class="sim-mode-btn" id="modeEmotes">League Emotes</button>
          </div>
        </div>
      </div>

      <div class="sim-workspace">
        <!-- Interactive Wheel Stage Card -->
        <div class="bezel">
          <div class="plate wheel-arena-card" id="arenaCard">
            <div class="dot-grid"></div>

            <button class="wheel-audio-toggle enabled" id="audioToggle">
              <svg width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/><path d="M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07"/></svg>
              <span>Sound ON</span>
            </button>

            <div class="wheel-container" id="wheelContainer">
              <svg id="wheelSvg" viewBox="0 0 460 460">
                <defs>
                  <filter id="hubShadow" x="-20%" y="-20%" width="140%" height="140%">
                    <feDropShadow dx="0" dy="4" stdDeviation="6" flood-opacity="0.3"/>
                  </filter>
                </defs>
                <!-- Outer Concentric Compass Ticks -->
                <circle cx="230" cy="230" r="224" fill="none" stroke="var(--tick)" stroke-width="1.2" stroke-dasharray="2 6"/>
                <circle cx="230" cy="230" r="216" fill="none" stroke="var(--line)" stroke-width="1"/>

                <!-- Dynamic Wedges Group -->
                <g id="wedgesGroup"></g>

                <!-- Active Highlight Arc -->
                <path id="highlightArc" class="highlight-arc" d="" opacity="0"/>

                <!-- Pointer Tracking Line -->
                <line id="dragLine" class="drag-indicator-line" x1="230" y1="230" x2="230" y2="230" opacity="0"/>

                <!-- Center 86px Cancellation Hub -->
                <circle id="hubCircle" class="hub-circle" cx="230" cy="230" r="86" filter="url(#hubShadow)"/>
                <circle cx="230" cy="230" r="82" fill="none" stroke="var(--shell-line)" stroke-width="1"/>
                <circle cx="230" cy="230" r="28" fill="none" stroke="var(--faint)" stroke-width="1.2" stroke-dasharray="3 3" opacity="0.45"/>

                <text x="230" y="222" text-anchor="middle" font-size="9" font-weight="800" fill="var(--faint)" letter-spacing="2">RELEASE</text>
                <text x="230" y="240" text-anchor="middle" font-size="12" font-weight="700" fill="var(--ink)">to cancel</text>
              </svg>
            </div>

            <div class="wheel-footer-hint">
              Drag outward past the hub • Release to trigger radial dispatch
            </div>
          </div>
        </div>

        <!-- Slice Inspector Side Editor -->
        <div class="bezel">
          <div class="plate slice-inspector">
            <div>
              <div class="inspector-header">
                <div class="active-badge-preview" id="inspectBadge">
                  <svg width="34" height="34" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" stroke-width="2.5"><polygon points="12 2 15 8 22 9 17 14 18 21 12 17 6 21 7 14 2 9 9 8 12 2"/></svg>
                </div>
                <div>
                  <div class="inspector-title" id="inspectTitle">Danger</div>
                  <div class="inspector-sub" id="inspectSub">North Slice • 0° Bearing</div>
                </div>
              </div>

              <div class="slice-properties" style="margin-top:20px;">
                <div class="prop-card">
                  <div class="prop-label">Gesture Vector</div>
                  <div class="prop-val" id="inspectVector">N (0°)</div>
                </div>
                <div class="prop-card">
                  <div class="prop-label">Chime Synth</div>
                  <div class="prop-val" id="inspectFreq">880 Hz</div>
                </div>
                <div class="prop-card">
                  <div class="prop-label">Threshold</div>
                  <div class="prop-val">86px Hub</div>
                </div>
                <div class="prop-card">
                  <div class="prop-label">Overlay Latency</div>
                  <div class="prop-val" style="color:var(--success);">&lt; 0.5ms</div>
                </div>
              </div>
            </div>

            <div class="sim-explanation">
              <strong>V7 Native Geometry Cache:</strong> The radial wheel geometry is computed once upon gesture initiation. The Win32 BGRA composition path and macOS CGEventTap render frames purely through cache interpolation, keeping input latency strictly sub-millisecond.
            </div>

            <div style="display:flex; gap:10px;">
              <a href="#downloads" class="btn btn-primary" style="flex:1;">Install Pingloom</a>
              <a href="${repo}" target="_blank" rel="noopener noreferrer" class="btn btn-secondary" style="flex:1;">View Code</a>
            </div>
          </div>
        </div>
      </div>
    </section>

    <!-- Architecture Bento Grid -->
    <section class="bento-section" id="architecture">
      <div class="section-eyebrow">
        <span class="eyebrow">SYSTEM ARCHITECTURE</span>
      </div>
      <h2 class="section-title">Built for Competitive Latency. Zero Bloat.</h2>

      <div class="bento-grid">
        <div class="bezel bento-card-wide">
          <div class="plate bento-card">
            <div class="bento-icon-box">
              <svg width="24" height="24" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>
            </div>
            <h3>Zero-Allocation Native Input Loop</h3>
            <p>
              Unlike electron apps running Node runtimes in the background, Pingloom's engine is written in pure Rust. Low-level Win32 hooks (<code>WH_MOUSE_LL</code>) and macOS <code>CGEventTap</code> execute without garbage collection pauses, heap reallocations, or thread synchronization locks in the gesture detection hot path.
            </p>
          </div>
        </div>

        <div class="bezel">
          <div class="plate bento-card">
            <div class="bento-icon-box">
              <svg width="24" height="24" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24"><circle cx="12" cy="12" r="10"/><path d="M8 14s1.5 2 4 2 4-2 4-2"/><line x1="9" y1="9" x2="9.01" y2="9"/><line x1="15" y1="9" x2="15.01" y2="9"/></svg>
            </div>
            <h3>48 League Emotes + Custom Media</h3>
            <p>
              Bundles 48 high-fidelity animated emotes decoded locally with hardware WebP rasterization. Supports drag-and-dropping your own GIFs, PNGs, and audio files with auto sidecar caching.
            </p>
          </div>
        </div>

        <div class="bezel">
          <div class="plate bento-card">
            <div class="bento-icon-box">
              <svg width="24" height="24" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 9h18"/><path d="M9 21V9"/></svg>
            </div>
            <h3>Tauri 2 + Svelte 5 Settings</h3>
            <p>
              The settings panel runs in a sandboxed, locally-bundled WebView2 context. Replaces heavy runtimes with Svelte 5 runes, instant appearance synchronization, and strict CSP restrictions.
            </p>
          </div>
        </div>

        <div class="bezel bento-card-wide">
          <div class="plate bento-card">
            <div class="bento-icon-box">
              <svg width="24" height="24" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
            </div>
            <h3>Autonomous Window &amp; Fullscreen Awareness</h3>
            <p>
              Guarded by kernel per-user lockfiles with clean unlinking on shutdown. Intelligently identifies fullscreen exclusive games, pauses gesture capture when appropriate, handles mixed-DPI multi-monitor bounds, and automatically releases stuck modifier keys.
            </p>
          </div>
        </div>
      </div>
    </section>

    <!-- Telemetry & Performance Gate -->
    <section class="telemetry-section" id="telemetry">
      <div class="bezel">
        <div class="plate" style="padding: 40px;">
          <div style="display:flex; justify-content:space-between; align-items:flex-end; flex-wrap:wrap; gap:20px;">
            <div>
              <div class="eyebrow" style="margin-bottom:8px;">CONTINUOUS VERIFICATION</div>
              <h2 style="font-family:var(--display); font-size:32px; font-weight:800; letter-spacing:-0.025em;">Measured CI Telemetry Gates</h2>
            </div>
            <p style="color:var(--muted); font-size:14px; max-width:440px;">
              Enforced by automated GitHub Actions CI gates on Windows 11 and macOS 15 runners. Every release validates latency, allocation, and render timings.
            </p>
          </div>

          <div class="telemetry-grid">
            <div class="metric-card">
              <div class="metric-value">0.02ms</div>
              <div class="metric-label">p95 Hook Latency</div>
              <div class="metric-desc">Sub-millisecond input capture</div>
            </div>
            <div class="metric-card">
              <div class="metric-value">0 B</div>
              <div class="metric-label">Hot-Loop Allocs</div>
              <div class="metric-desc">Zero heap churn during gestures</div>
            </div>
            <div class="metric-card">
              <div class="metric-value">&lt; 35 MB</div>
              <div class="metric-label">Idle Footprint</div>
              <div class="metric-desc">Lightweight system tray presence</div>
            </div>
            <div class="metric-card">
              <div class="metric-value">240 Hz</div>
              <div class="metric-label">Display Sync</div>
              <div class="metric-desc">Seamless high-refresh monitor support</div>
            </div>
          </div>
        </div>
      </div>
    </section>

    <!-- Downloads Matrix -->
    <section class="downloads-section" id="downloads">
      <div class="section-eyebrow">
        <span class="eyebrow">OFFICIAL RELEASES</span>
      </div>
      <h2 class="section-title">Get Pingloom v${version}</h2>

      <div class="downloads-grid">
        <!-- Windows Card -->
        <div class="bezel">
          <div class="plate" style="display:flex; flex-direction:column; justify-content:space-between; height:100%;">
            <div>
              <div class="dl-platform-header">
                <div class="dl-platform-icon">
                  <svg width="26" height="26" fill="currentColor" viewBox="0 0 24 24"><path d="M0 3.449L9.75 2.1v9.451H0m10.949-9.602L24 0v11.4H10.949M0 12.6h9.75v9.451L0 20.699M10.949 12.6H24V24l-12.9-1.801"/></svg>
                </div>
                <div class="dl-platform-title">
                  <h4>Windows</h4>
                  <span>Windows 10 / 11 (64-bit)</span>
                </div>
              </div>
              <p style="color:var(--muted); font-size:14px; line-height:1.6;">
                Full native Win32 low-level hook owner with DirectComposition overlay, tray icon, and Evergreen WebView2 settings.
              </p>
            </div>
            <div class="dl-buttons-row">
              <a href="/download/windows" class="btn btn-primary" style="flex:1;">
                Installer (.exe)
              </a>
              <a href="/download/windows-portable" class="btn btn-secondary" style="flex:1;">
                Portable (.exe)
              </a>
            </div>
          </div>
        </div>

        <!-- macOS Card -->
        <div class="bezel">
          <div class="plate" style="display:flex; flex-direction:column; justify-content:space-between; height:100%;">
            <div>
              <div class="dl-platform-header">
                <div class="dl-platform-icon">
                  <svg width="26" height="26" fill="currentColor" viewBox="0 0 24 24"><path d="M18.71 19.5c-.83 1.24-1.71 2.45-3.05 2.47-1.34.03-1.77-.79-3.29-.79-1.53 0-2 .77-3.27.82-1.31.05-2.3-1.32-3.14-2.53C4.25 17 2.94 12.45 4.7 9.39c.87-1.52 2.43-2.48 4.12-2.51 1.28-.02 2.5.87 3.29.87.78 0 2.26-1.07 3.81-.91.65.03 2.47.26 3.64 1.98-.09.06-2.17 1.28-2.15 3.81.03 3.02 2.65 4.03 2.68 4.04-.03.07-.42 1.44-1.38 2.83M15.97 4.88c.67-.82 1.13-1.96.99-3.11-1 .04-2.17.67-2.88 1.49-.63.73-1.18 1.9-1.03 3.01 1.12.09 2.24-.57 2.92-1.39z"/></svg>
                </div>
                <div class="dl-platform-title">
                  <h4>macOS Universal</h4>
                  <span>Apple Silicon &amp; Intel (macOS 12+)</span>
                </div>
              </div>
              <p style="color:var(--muted); font-size:14px; line-height:1.6;">
                Universal application bundle running CoreGraphics CGEventTap and native CGImage bitmap presentation.
              </p>
            </div>
            <div class="dl-buttons-row">
              <a href="/download/macos" class="btn btn-primary" style="flex:1;">
                Download Pingloom.app (.zip)
              </a>
            </div>
          </div>
        </div>
      </div>
    </section>

    <!-- Edge-rendered Footer -->
    <footer>
      <div>
        <strong>Pingloom</strong> &copy; 2026. Released under the MIT License.
      </div>
      <div style="display:flex; align-items:center; gap:16px;">
        <span class="edge-badge">Cloudflare Edge [${edgeRegion}]</span>
        <a href="${repo}" target="_blank" rel="noopener noreferrer">GitHub</a>
        <a href="${repo}/releases" target="_blank" rel="noopener noreferrer">Releases</a>
        <a href="/api/status">API</a>
      </div>
    </footer>
  </main>

  <script>
    // Specular Pointer Light on Double Bezels
    document.querySelectorAll('.bezel').forEach(bezel => {
      bezel.addEventListener('pointermove', e => {
        const rect = bezel.getBoundingClientRect();
        bezel.style.setProperty('--mx', (e.clientX - rect.left) + 'px');
        bezel.style.setProperty('--my', (e.clientY - rect.top) + 'px');
      });
      bezel.addEventListener('pointerleave', () => {
        bezel.style.setProperty('--mx', '-999px');
        bezel.style.setProperty('--my', '-999px');
      });
    });

    // Theme Switcher System
    const themeSelect = document.getElementById('themeSelect');
    const savedTheme = localStorage.getItem('pingloom_theme') || 'default-dark';
    if (savedTheme) {
      document.documentElement.dataset.themePalette = savedTheme;
      document.documentElement.dataset.themeMode = savedTheme.includes('light') || savedTheme === 'latte' ? 'light' : 'dark';
      if (themeSelect) themeSelect.value = savedTheme;
    }
    themeSelect.addEventListener('change', (e) => {
      const val = e.target.value;
      document.documentElement.dataset.themePalette = val;
      document.documentElement.dataset.themeMode = val.includes('light') || val === 'latte' ? 'light' : 'dark';
      localStorage.setItem('pingloom_theme', val);
    });

    // WebAudio Synthesizer
    let audioCtx = null;
    let audioEnabled = true;
    const audioToggle = document.getElementById('audioToggle');
    audioToggle.addEventListener('click', () => {
      audioEnabled = !audioEnabled;
      audioToggle.classList.toggle('enabled', audioEnabled);
      audioToggle.querySelector('span').textContent = audioEnabled ? 'Sound ON' : 'Sound OFF';
    });

    function playTactileChime(freq) {
      if (!audioEnabled) return;
      try {
        if (!audioCtx) audioCtx = new (window.AudioContext || window.webkitAudioContext)();
        if (audioCtx.state === 'suspended') audioCtx.resume();
        const now = audioCtx.currentTime;

        const osc = audioCtx.createOscillator();
        const subOsc = audioCtx.createOscillator();
        const gain = audioCtx.createGain();

        osc.type = 'triangle';
        osc.frequency.setValueAtTime(freq, now);

        subOsc.type = 'sine';
        subOsc.frequency.setValueAtTime(freq * 0.5, now);

        gain.gain.setValueAtTime(0.08, now);
        gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.16);

        osc.connect(gain);
        subOsc.connect(gain);
        gain.connect(audioCtx.destination);

        osc.start(now);
        subOsc.start(now);
        osc.stop(now + 0.16);
        subOsc.stop(now + 0.16);
      } catch (e) {}
    }

    // Wheel Slice Catalogues
    const PINGS_DATA = [
      { name: 'Danger', bearing: 'N (0°)', color: '#ff4d61', freq: 659, icon: 'M12 9v4m0 4h.01M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z' },
      { name: 'Enemy Vision', bearing: 'NE (45°)', color: '#ff6b4a', freq: 740, icon: 'M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z' },
      { name: 'On My Way', bearing: 'E (90°)', color: '#2e8bff', freq: 830, icon: 'M13 5l7 7-7 7M5 5l7 7-7 7' },
      { name: 'All In', bearing: 'SE (135°)', color: '#f2b320', freq: 987, icon: 'M14.5 17.5L3 6V3h3l11.5 11.5 M13 19l6-6' },
      { name: 'Assist Me', bearing: 'S (180°)', color: '#35d27a', freq: 784, icon: 'M4 15s1-1 4-1 5 2 8 2 4-1 4-1V3s-1 1-4 1-5-2-8-2-4 1-4 1z M4 22v-7' },
      { name: 'Need Vision', bearing: 'SW (225°)', color: '#26c281', freq: 698, icon: 'M2 12h20 M12 2v20 M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8z' },
      { name: 'Enemy Missing', bearing: 'W (270°)', color: '#f2c518', freq: 587, icon: 'M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3 M12 17h.01' },
      { name: 'Push', bearing: 'NW (315°)', color: '#45b6fe', freq: 880, icon: 'M5 12h14 M12 5l7 7-7 7' }
    ];

    const EMOTES_DATA = [
      { name: 'Angry Kitty', bearing: 'N (0°)', color: '#ff5c5c', freq: 659, icon: 'M12 2a10 10 0 1 0 10 10A10 10 0 0 0 12 2zm-3 8a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3zm6 0a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3zm-6 6a6 6 0 0 1 6 0' },
      { name: 'Bee Happy', bearing: 'NE (45°)', color: '#f7d02c', freq: 740, icon: 'M12 2a10 10 0 1 0 10 10A10 10 0 0 0 12 2zm-4 7h1m6 0h1m-8 4a5 5 0 0 0 8 0' },
      { name: 'GG Heart', bearing: 'E (90°)', color: '#ff6584', freq: 830, icon: 'M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z' },
      { name: 'Despair', bearing: 'SE (135°)', color: '#6c7a89', freq: 987, icon: 'M12 2a10 10 0 1 0 10 10A10 10 0 0 0 12 2zm-4 7a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3zm8 0a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3zm-4 8a4 4 0 0 1-4-3h8a4 4 0 0 1-4 3z' },
      { name: 'Poro Snax', bearing: 'S (180°)', color: '#f39c12', freq: 784, icon: 'M12 2L2 7l10 5 10-5-10-5z M2 17l10 5 10-5 M2 12l10 5 10-5' },
      { name: 'Scout Approved', bearing: 'SW (225°)', color: '#2ecc71', freq: 698, icon: 'M14 9V5a3 3 0 0 0-3-3l-4 9v11h11.28a2 2 0 0 0 2-1.7l1.38-9a2 2 0 0 0-2-2.3zM7 22H4a2 2 0 0 1-2-2v-7a2 2 0 0 1 2-2h3' },
      { name: 'Sweating Bot', bearing: 'W (270°)', color: '#3498db', freq: 587, icon: 'M12 2v20 M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6' },
      { name: 'Nice Try', bearing: 'NW (315°)', color: '#9b59b6', freq: 880, icon: 'M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z' }
    ];

    let currentMode = 'pings';
    let currentSlices = PINGS_DATA;
    let activeSlot = null;

    const wedgesGroup = document.getElementById('wedgesGroup');
    const arenaCard = document.getElementById('arenaCard');
    const hubCircle = document.getElementById('hubCircle');
    const highlightArc = document.getElementById('highlightArc');
    const dragLine = document.getElementById('dragLine');

    const inspectTitle = document.getElementById('inspectTitle');
    const inspectSub = document.getElementById('inspectSub');
    const inspectVector = document.getElementById('inspectVector');
    const inspectFreq = document.getElementById('inspectFreq');
    const inspectBadge = document.getElementById('inspectBadge');

    const cx = 230, cy = 230, rIn = 90, rOut = 212;

    function buildSectorPath(aStart, aEnd) {
      const p = (r, a) => [cx + r * Math.sin(a), cy - r * Math.cos(a)];
      const [p1x, p1y] = p(rIn, aStart);
      const [p2x, p2y] = p(rOut, aStart);
      const [p3x, p3y] = p(rOut, aEnd);
      const [p4x, p4y] = p(rIn, aEnd);
      return \`M \${p1x} \${p1y} L \${p2x} \${p2y} A \${rOut} \${rOut} 0 0 1 \${p3x} \${p3y} L \${p4x} \${p4y} A \${rIn} \${rIn} 0 0 0 \${p1x} \${p1y} Z\`;
    }

    function renderWheelSlices() {
      wedgesGroup.innerHTML = '';
      currentSlices.forEach((slice, i) => {
        const aStart = i * (Math.PI / 4) - Math.PI / 8;
        const aEnd = aStart + Math.PI / 4;

        // Sector shape
        const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
        path.setAttribute('d', buildSectorPath(aStart, aEnd));
        path.classList.add('radial-wedge');
        path.dataset.slot = i;

        // Label position
        const midAngle = (aStart + aEnd) / 2;
        const lx = cx + 155 * Math.sin(midAngle);
        const ly = cy - 155 * Math.cos(midAngle);

        const text = document.createElementNS('http://www.w3.org/2000/svg', 'text');
        text.setAttribute('x', lx);
        text.setAttribute('y', ly + 4);
        text.setAttribute('text-anchor', 'middle');
        text.setAttribute('font-size', '10.5');
        text.setAttribute('font-weight', '750');
        text.setAttribute('fill', 'var(--ink)');
        text.textContent = slice.name;
        text.style.pointerEvents = 'none';

        wedgesGroup.appendChild(path);
        wedgesGroup.appendChild(text);
      });
    }

    renderWheelSlices();

    // Mode Toggle Logic
    const modePingsBtn = document.getElementById('modePings');
    const modeEmotesBtn = document.getElementById('modeEmotes');

    modePingsBtn.addEventListener('click', () => {
      currentMode = 'pings';
      currentSlices = PINGS_DATA;
      modePingsBtn.classList.add('active');
      modeEmotesBtn.classList.remove('active');
      renderWheelSlices();
      updateInspector(0);
    });
    modeEmotesBtn.addEventListener('click', () => {
      currentMode = 'emotes';
      currentSlices = EMOTES_DATA;
      modeEmotesBtn.classList.add('active');
      modePingsBtn.classList.remove('active');
      renderWheelSlices();
      updateInspector(0);
    });

    function updateInspector(slot) {
      if (slot === null || slot < 0 || slot >= currentSlices.length) return;
      const s = currentSlices[slot];
      inspectTitle.textContent = s.name;
      inspectSub.textContent = \`Slot \${slot + 1} • \${s.bearing}\`;
      inspectVector.textContent = s.bearing;
      inspectFreq.textContent = \`\${s.freq} Hz\`;

      inspectBadge.innerHTML = \`<svg width="34" height="34" viewBox="0 0 24 24" fill="none" stroke="\${s.color}" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="\${s.icon}"/></svg>\`;
    }

    function setActiveSlot(slot, pointerX = cx, pointerY = cy) {
      if (activeSlot === slot) return;
      activeSlot = slot;

      const wedges = wedgesGroup.querySelectorAll('.radial-wedge');
      wedges.forEach((w, idx) => {
        if (idx === slot) {
          w.classList.add('active');
          w.style.fill = currentSlices[slot].color + '33';
          w.style.stroke = currentSlices[slot].color;
        } else {
          w.classList.remove('active');
          w.style.fill = '';
          w.style.stroke = '';
        }
      });

      if (slot !== null) {
        hubCircle.classList.remove('cancelling');
        const aStart = slot * (Math.PI / 4) - Math.PI / 8;
        const aEnd = aStart + Math.PI / 4;
        const p1 = [cx + (rOut + 2) * Math.sin(aStart), cy - (rOut + 2) * Math.cos(aStart)];
        const p2 = [cx + (rOut + 2) * Math.sin(aEnd), cy - (rOut + 2) * Math.cos(aEnd)];
        highlightArc.setAttribute('d', \`M \${p1[0]} \${p1[1]} A \${rOut + 2} \${rOut + 2} 0 0 1 \${p2[0]} \${p2[1]}\`);
        highlightArc.setAttribute('stroke', currentSlices[slot].color);
        highlightArc.setAttribute('opacity', '1');

        dragLine.setAttribute('x2', pointerX);
        dragLine.setAttribute('y2', pointerY);
        dragLine.setAttribute('stroke', currentSlices[slot].color);
        dragLine.setAttribute('opacity', '0.7');

        updateInspector(slot);
        playTactileChime(currentSlices[slot].freq);
      } else {
        hubCircle.classList.add('cancelling');
        highlightArc.setAttribute('opacity', '0');
        dragLine.setAttribute('opacity', '0');
      }
    }

    // Touch and Pointer Drag Handling
    let isDragging = false;
    function handleWheelPointer(e) {
      const rect = document.getElementById('wheelSvg').getBoundingClientRect();
      const clientX = e.clientX || (e.touches && e.touches[0].clientX);
      const clientY = e.clientY || (e.touches && e.touches[0].clientY);

      const svgScale = 460 / rect.width;
      const x = (clientX - rect.left) * svgScale - cx;
      const y = (clientY - rect.top) * svgScale - cy;
      const dist = Math.hypot(x, y);

      if (dist < 86) {
        setActiveSlot(null);
        return;
      }
      let deg = (Math.atan2(y, x) * 180 / Math.PI + 90 + 22.5 + 360) % 360;
      const slot = Math.floor(deg / 45) % 8;
      setActiveSlot(slot, cx + x, cy + y);
    }

    arenaCard.addEventListener('pointerdown', (e) => {
      isDragging = true;
      handleWheelPointer(e);
    });
    window.addEventListener('pointermove', (e) => {
      if (isDragging) handleWheelPointer(e);
    });
    window.addEventListener('pointerup', (e) => {
      if (isDragging && activeSlot !== null) {
        // Ping Ripple explosion
        const rect = arenaCard.getBoundingClientRect();
        const clientX = e.clientX || (e.changedTouches && e.changedTouches[0].clientX) || (rect.left + rect.width / 2);
        const clientY = e.clientY || (e.changedTouches && e.changedTouches[0].clientY) || (rect.top + rect.height / 2);

        const ripple = document.createElement('div');
        ripple.className = 'ping-ripple';
        ripple.style.setProperty('--ripple-color', currentSlices[activeSlot].color);
        ripple.style.left = (clientX - rect.left) + 'px';
        ripple.style.top = (clientY - rect.top) + 'px';
        arenaCard.appendChild(ripple);
        setTimeout(() => ripple.remove(), 750);
      }
      isDragging = false;
      setActiveSlot(null);
      hubCircle.classList.remove('cancelling');
    });
  </script>
</body>
</html>
`;
}
