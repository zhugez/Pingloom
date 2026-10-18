# Pingloom Landing Page (Cloudflare Worker)

High-performance, edge-rendered landing page for **Pingloom** designed with the authentic **Loom V7 Design System** (Loom Day / Loom Night / Catppuccin palettes, double-bezel specular reflections, tactile radial wheel simulator with WebAudio synthesis, and platform download routing).

- **Production URL**: [https://app.pingloom.workers.dev/](https://app.pingloom.workers.dev/)
- **Runtime**: Cloudflare Workers
- **Worker Name**: `app`

---

## Project Structure

```
landing/
├── wrangler.toml       # Cloudflare Workers configuration (workers_dev = true)
├── package.json        # NPM configuration (ESM)
├── src/
│   └── index.js        # Worker script serving edge-rendered landing page & API
└── README.md           # Documentation & CI/CD deployment guide
```

---

## Design System (Loom V7 Port)

The landing page mirrors the desktop Tauri 2 Settings UI design:

1. **Loom Day / Loom Night & Catppuccin Color Schemes**:
   - `Loom Night`: Dark ground (`#121013`), plate (`#1a171b`), Pingloom Blush accent (`#e2b4bd`), ink (`#fff5f5`).
   - `Loom Day`: Light ground (`#fbf6f5`), crisp white plates, blush secondary surfaces, ink (`#3a3437`).
   - `Catppuccin`: Canonical Latte, Frappé, and Mocha palettes.
2. **Double-Bezel Specular Reflection**:
   - Outer shell (`.bezel`) with 36px radius and interactive specular spotlight tracking pointer movement (`--mx`, `--my`).
   - Inner plate (`.plate`) with 30px radius and fine inset highlight (`inset 0 1px 0 var(--hi)`).
3. **Interactive Dual-Mode Radial Wheel Simulator**:
   - 8 tactical pings: *Danger, Enemy Vision, On My Way, All In, Assist Me, Need Vision, Enemy Missing, Push*.
   - 8 League emotes: *Angry Kitty, Bee Happy, GG Heart, Despair, Poro Snax, Scout Approved, Sweating Bot, Nice Try*.
   - Web Audio API synthesizer with harmonic overtones and sound toggle.
   - 86px central cancellation hub ("RELEASE to cancel") with dead-zone detection.
   - Dynamic radial burst ping explosion on release.
4. **Interactive Slice Inspector**:
   - Live property breakdown (angle, bearing, audio frequency, threshold).
5. **Continuous Telemetry & Hardware CI Gate**:
   - Highlights sub-millisecond p95 latency (`0.02ms`), zero heap churn (`0 B`), and lightweight memory footprint (`< 35 MB`).

---

## Edge Endpoints & Redirects

| Route | Description |
|---|---|
| `/` | Edge-rendered HTML landing page with Cloudflare cache headers |
| `/api/status` | JSON endpoint returning Pingloom version, build health, and release URLs |
| `/download/windows` | 302 redirect to Windows NSIS setup executable (`Pingloom_0.1.0_x64-setup.exe`) |
| `/download/windows-portable` | 302 redirect to portable standalone executable (`pingloom.exe`) |
| `/download/macos` | 302 redirect to macOS universal zip bundle (`pingloom-macos-universal.zip`) |

---

## Local Development

```bash
cd landing
npm install
npx wrangler dev
```

Test dry-run bundle:
```bash
npx wrangler deploy --dry-run
```

---

## CI/CD Deployment to Cloudflare Workers

Automated deployment is configured in `.github/workflows/deploy-landing.yml`.

### Deployment Target
- **URL**: `https://app.pingloom.workers.dev/`
- **Worker**: `app` on the `pingloom` workers.dev subdomain.

### Required GitHub Secrets
Configure these in **GitHub Repository Settings → Secrets and variables → Actions**:

1. `CLOUDFLARE_API_TOKEN`: Cloudflare API token with **Workers:Edit** permission.
2. `CLOUDFLARE_ACCOUNT_ID` *(optional / recommended)*: Your Cloudflare account ID.

### Workflow Triggers
- Automatic deployment on `push` to `main` modifying `landing/**` or `.github/workflows/deploy-landing.yml`.
- Dry-run validation on `pull_request`.
- Manual trigger via `workflow_dispatch`.
