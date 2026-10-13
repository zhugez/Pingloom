# ADR 0001: Local Settings WebView, native gesture owner

- Status: Accepted; integrated, Windows runtime verification outstanding
- Date: 2026-02-01

## Context
The approved Settings frontend uses Svelte 5/Vite/Tailwind/daisyUI in a Tauri 2 desktop shell. Gesture overlays, hooks, replay, animation/audio and tray are latency-sensitive and already implemented natively. Replacing them with WebViews would risk latency and input behavior. Previous docs prohibited all WebViews; the user explicitly superseded that restriction for Settings only.

## Decision
Use one bundled local Settings WebView, with Tauri owning its UI loop. Keep hooks and all gesture/runtime state on a separate pumped Win32 owner thread. Communicate via bounded, owned, validated commands and replies; only that owner validates/applies/persists settings. Require WebView2 Evergreen on Windows 11. Restrict the capability to Settings and a local origin; no remote navigation, server, shell/filesystem plugin, or arbitrary paths. Native overlays and tray remain native.

## Consequences
Node/npm are build-time only; Vite output is bundled as local files. Embedded assets already total about 58 MiB, so a tiny 2–4 MiB executable is not credible. Settings suspension must acknowledge replay before display and resume idempotently on close/failure. Tauri/WebView2 lifecycle, renderer recovery and all Windows behavior require real Windows verification.

## Implementation status
Tauri config/bootstrap, scoped capability, local navigation/CSP, bundled UI, command bridge and owner mailbox are integrated. The active tray path uses the WebView Settings window; native Settings source remains dormant. Host checks and Windows-target compile/link evidence do not certify behavior on Windows. Evergreen WebView2 startup, close/watchdog/renderer failure, orderly shutdown, DPI and visual/accessibility behavior still require the Windows smoke checklist; do not interpret integration as runtime acceptance.
