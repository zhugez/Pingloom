# Implementation ownership

Classification: multi-seam. Portable gesture/settings behavior can be tested independently of Win32 hook/window/render APIs. Native input, message pump, overlay and settings UI are tightly coupled by ownership and event dispatch, so keep them one native component owner, not overlapping window writers.

All lanes run serially in /home/dev/lolping-rs on main. Each component commits its durable handoff before the next owner starts. Parent owns publication, scope and acceptance. No child may push, create remote repos, release, spawn other agents, or change approved scope.

| Lane | Exclusive ownership | Gate | Handoff | Independence |
| --- | --- | --- | --- | --- |
| Core | Cargo.toml/Cargo.lock bootstrap, src/lib.rs and portable model/gesture/settings/geometry modules | cargo test, fmt, clippy portable logic | Commit + managed report with public API and test commands | No Windows desktop needed |
| Native | src/main.rs, src/windows/**, Windows-only dependencies/resources, native settings/hooks/tray/overlay/audio, assets manifest/packaging | Windows target compile; core tests remain passing | Commit + managed report, exact runtime checks not run | Consumes core API; does not rewrite core semantics |
| Integration | Windows CI/build packaging/README, minimal fixes at component boundary only | fmt/test/clippy and real Windows target compile or CI | Commit + exact validation evidence | After both contracts exist; does not reimplement components |
| Review | Read-only source/contract review | Evidence-backed findings | Managed report | Fresh inspection after integration |

Upstream source is /tmp/lolping-upstream at f24a51e5a11c2b31dab72d89a571cc4025ad3c98, read-only. Generated branding is parent-owned before workers launch. Parent must inspect final findings and disposition them before reporting completion.
