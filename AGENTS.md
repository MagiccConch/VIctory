# VIctory — agent notes

Fork: MagiccConch/VIctory, upstream: Marvisak/VIctory. CI: windows-latest only (Clippy + Tests). Linux container cannot compile most `cfg(windows)` code.

## Environment and verification

- `cargo fmt` churns ~49 files at once (rustfmt.toml: max_width=100, field-init shorthand). **Never run bare `cargo fmt --all` before committing**; it discards nothing but buries real edits in noise. Restore untouched files from HEAD before staging: `git restore --staged --worktree <files>` then re-apply only semantic hunks.
- **Always `git add` immediately after editing a file.** Unstaged edits have been lost twice by cleanup sequences (`git checkout -- .` and `git checkout-index -f -a`).

## Hardware testing session (2026-11-25)

- User hardware: NVIDIA RTX 5080 + AMD Radeon iGPU (hybrid), Windows 10 Home 22H2 (10.0.19045). PC paths: clone `C:\Users\Who\VIctory`, dump `C:\Users\Who\Downloads\GTA VI`, exe `target\release\victory.exe`.
- Pre-build refusal report (all 28164 pipelines, 0 made): 15863 root signature E_INVALIDARG (0x80070057) — Win10 inbox runtime refuses the 0x400/0x800 heap-directly-indexed flags; 11166 "no mesh shaders (or no SM 6.6)" — device was on the iGPU, not the 5080 (detection is per-device, `victory-gpu/src/host.rs::mesh_shaders()`); 994 unnamed states (852 rasteriser, 139 depth-stencil, 3 blend), 101 untranslated (61 compute, 40 pixel), 30 no 64-bit typed heap atomics, 8 shader not found, 2 exotic stages.
- Commit `9d5b5ce` (CI 45521142787 green): (1) `victory-gpu/src/host.rs::by_preference()` — pick adapter via IDXGIFactory6 EnumAdapterByGpuPreference HIGH_PERFORMANCE, `VICTORY_ADAPTER` substring override, WARP-guarded, fallback to null-adapter with a warn; (2) Agility opt-in: `apps/victory/src/main.rs` exports `D3D12SDKVersion: u32 = 620` and `D3D12SDKPath: &[u8; 9] = b".\\D3D12\\\0"` (cfg windows, no_mangle), `apps/victory/build.rs` `/EXPORT` link args gated on target os. windows crate 0.62.2: enum is `DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE` (not DXGI_ADAPTER_PREFERENCE...), `EnumAdapterByGpuPreference::<IDXGIAdapter1>` generic; clippy (windows-gnu) requires the whole body in one unsafe block with one SAFETY comment and no nested unsafe (nested = unused_unsafe error).
- windows crate gotcha: `D3D12CreateDevice` Param<IDXGIAdapter> generics did not accept the call-site IDXGIAdapter1 needing explicit cast in an early draft; final code passes `&adapter` where adapter: IDXGIAdapter1 — check compiles both check + clippy before pushing (windows-gnu per-crate type-checks victory-gpu and victory).
- User still must copy `D3D12Core.dll` + `D3D12SDKLayers.dll` (Microsoft.Direct3D.D3D12 1.620.2 NuGet, build/native/bin/x64) into `target\release\D3D12\` next to victory.exe — the exports only opt in; the DLL ships the runtime.
- Next: user re-pull, rebuild, re-run pre-build; expect the 15863 root-signature and 11166 mesh refusals to collapse. Pipeline library started empty this run (earlier 2147-made library not found) — verify it saves once pipelines are made. Remaining code gaps: 994 unnamed states, 101 untranslated shaders, 30 atomics refusals, 8 shader-not-found, 2 exotic stages.

- To type-check Windows-only code from Linux: `rustup target add x86_64-pc-windows-gnu`, then `cargo check/clippy -p <crate> --all-features --all-targets --target x86_64-pc-windows-gnu`. This parses and type-checks all `cfg(windows)` modules (incl. test-only ones). It compiles clean for victory-shader and victory-gpu; the workspace-wide Windows check needs MSVC on real CI.
- Linux `cargo check --workspace` fails in victory-gpu/victory-hle (`E0425 host_resource` in resource.rs:284, `#[cfg(windows)]` fn at ~line 986) — pre-existing in both parents, not a merge artifact. Scope Linux checks per-crate: `cargo check -p victory-core -p victory-pipeline -p victory-pe -p victory-loader -p victory-shader -p victory-game --all-targets`.

## Upstream/CI state (as of 2026-11-24)

- Upstream main (70564ae) is RED on its own CI: `emit::synthetic_tests::gathers_at_a_level_match_the_hardware` (hardware self-inconsistency: left [66,67,35,34] vs right [65,66,34,33] on runner GPU) and `emit::synthetic_tests::permutes_read_the_lane_named_and_zero_from_lanes_left_out` ("no pipeline for a wave of 32" on hardware AND warp, then `no device ran the permute`). Last green upstream main: ffe310eb (the fork's merge-base). Both failures are runner-GPU-dependent; do NOT treat as merge regressions.
- **Fork resolution (option b, chosen by user):** both tests carry `#[ignore = "…"]` in commit 5002034. CI is GREEN since run 45505795702. On real hardware run them with `cargo test -- --ignored`. When upstream fixes their red (watch Marvisak/VIctory main runs; ffe310eb..70564ae added the tests), re-sync and drop the two ignores.
- Fork main history: 4af1bf0 merge (8 conflicts), 088b2d3 config.rs `fidelityfx_dir: None` in policy tests, 9ab37e9 windows-only initializer completion (valu_tests build_stage HostShader from `lowered.*`; fmad probe passes `name: adapter_name(&device)`; helper `adapter_name` extracted in emit/host.rs).
- `Config` (victory-core/src/config.rs) has `fidelityfx_dir: Option<PathBuf>` from upstream — all struct literals must list it.
- `emit::host::Host` (fork) carries `name: String` in addition to upstream's `device`; `Host::new` fills it via DXGI. Upstream code that constructs `Host { device }` directly must pass `name` (see `adapter_name` helper).

## Local hardware testing state (user's PC, 2026-11-24)

- User's PC: toolchain installed (repo pin 1.98.1 via rustup; default 1.100.0 elsewhere), full workspace builds, 200+ tests pass locally. `cargo test --workspace` fails only 3 `victory-gpu` tests — all **upstream-new tests from the merge** that encode an uncapped GPU:
  - `dynamic_slots_are_held_until_released` (descriptor/host.rs:1537): upstream turned the fork's pure-logic test into one asserting `NumDescriptors >= HEAP_VIEWS` (2M). On GPUs whose driver caps shader-visible heaps (NVIDIA-style), `make((count+DYNAMIC).max(views))` fails and the documented fallback (`count + MIN_DYNAMIC`, 64K) runs → assert false. CI runner GPU accepts 2M → green there.
  - `lists_share_a_table_and_copies_keep_their_views` (memory.rs:755): asserts copy view pairs land `>= 1 << 18` (`DYNAMIC`); on a capped heap only `MIN_DYNAMIC = 1 << 16` dynamic slots exist → same root cause.
  - `a_texture_written_in_one_format_reads_in_another_through_a_placed_view` (draw.rs:2136): expects castable-format sharing (`ptr_eq`); user's GPU produced two textures. Castable lists are computed statically (`resource.rs castable()`); sharing depends on the driver accepting the castable set at creation. Root cause not fully pinned.
- The fork's runtime handles capped heaps by design (fallbacks + clamp); these failures do NOT block building or running the title. Decision offered to user: hardware-aware test patch vs leave failing vs ignore. GPU model requested to confirm (Get-CimInstance win32_VideoController).
- Local run flow (README): `cargo build --release -p victory`, then `target\release\victory.exe translate --cache --pipelines <game dir>\shaders.rpf <exe> <dlls>`, then `run <exe> --execute`. Config: copy victory.example.toml → victory.toml, `cargo run -p victory -- config` validates paths (asset policy refuses paths inside the repo). Needs AVX2 CPU, SM 6.6 + mesh shaders GPU, several GiB Windows commit (error 1455 → paging file). Real-GPU hazard: world entry has hung GPUs before (driver reset); WARP fallback via VICTORY_D3D12_WARP=1 (diagnostics builds only).

## Pre-build refusal report (commit 8281f7a, 2026-11-24)

- `victory-gpu/src/pipeline/host.rs`: thread_local `REFUSAL` records why the last `HostPipeline::new` on the thread made nothing; cleared at the top of `new()`, set by legacy `refuse()` and by `refused(what, e, shaders)` at the 4 device-creation failures (root signature ~493, compute ~522, mesh ~555, graphics ~630). `refusal()` (pub(crate)) reads it.
- `victory-gpu/src/pipeline.rs` `Prebuild::make` (~659): the `None` arm's Err now prefers `host::refusal()` over the flat "the host refuses it".
- Consumer: `apps/victory/src/prebuild.rs` `skip(why)` aggregates into a `BTreeMap`, printed at pre-build end as `N skipped: <cause>` lines — so a finished pre-build IS the per-cause GPU/driver support report.
- Pre-build resumes: pipelines already in the library (`<cache>/pipelines`) are listed, not remade.
- The two ignored tests run on real hardware with `cargo test -- --ignored` (commit 5002034): `gathers_at_a_level_match_the_hardware` (victory-shader emit) and `permutes_read_the_lane_named_and_zero_from_lanes_left_out` (needs wave-32 GPU).

## Merge resolution conventions used (4af1bf0)

- victory-gpu descriptor/host.rs, draw.rs: took upstream rewrites.
- victory-shader emit tests: kept both suites (fork + upstream) where compatible.
- CI job: "check" — steps: diagnostics build, Clippy, Tests (in that order).

- Title run (2026-11-25 ~02:17): title boots deep into init on the 5080, then 0xc0000005 null write (title-image+0x2215c35) amid a flood of "title slot past the host heap index~1179606 count=934464 for_draw=false" (also the title's own Breakpad minidump upload attempt, blocked by net HLE - benign). Root cause: the title requests 0x120000 (1179648) shader-visible slots; HostHeap::new fallback jumped from make(max(count+DYNAMIC, HEAP_VIEWS)=2M) straight to the 1M cap (PORTABLE_VIEWS, NVIDIA), leaving count=934464 and every title slot past it null. Commit 6ac8fbe (CI 45532159717 green): ordered fallbacks - try make(count+DYNAMIC) then make(count+MIN_DYNAMIC) before the PORTABLE_VIEWS cap, so the full console heap fits when the host allows ~1.245M. If the 5080 still caps (log shows host view heap capped at 1000000), real remapping is needed (upstream comment: a title that reaches them needs remapping). Breakpad line = the title's own crash reporter trying submit.backtrace.io, net-blocked, harmless.
