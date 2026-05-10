# Claude guidance for the Solitaire repo

## Architecture invariants

- 4-crate workspace: `solitaire-core` (logic+widgets), `solitaire-native` (winit + wgpu), `solitaire-wasm` (cdylib), `demo/` (TS shell). Mirrors `agg-gui`'s demo-* pattern and the sibling `Antidote` repo.
- `solitaire-core` MUST stay `wasm32`-clean. No `tokio`, no `dotenvy`, no `dirs`, no `winit`, no `wgpu`. Both shells inject services through traits in `solitaire_core::platform`.
- `solitaire-native` and `solitaire-wasm` are **platform shells only**. They wire up the OS/browser window or canvas, wgpu surface, event loop, input forwarding, and platform persistence. They contain **no game or UI content**: every game rule, widget tree, menu, layout, HUD, dialog, leaderboard, and interface the user sees is shared via `solitaire-core`. Platform crates call shared builders such as `solitaire_core::ui::build_solitaire_app()` and forward events; they never construct screens or widgets directly.
- Platform split, copied from agg-gui's goal:
  - **Game / widget / layout code** → `solitaire-core`
  - **GPU renderers (WGSL shaders, geometry, draw calls)** → `demo-wgpu` / future `agg-gui` wgpu backend
  - **Platform shell (OS window or browser canvas + event forwarding + persistence backend)** → `solitaire-native` and `solitaire-wasm`
- agg-gui is Y-up first-quadrant. All widget code uses Y-up; the OS event Y-flip happens once inside `App::on_mouse_*`. Keep card layout math in Y-up.
- DB access goes through Supabase REST (PostgREST + `/auth/v1/*`) over `ehttp`. No direct Postgres connection — wouldn't work in WASM.
- Anon key ships in the build artifact; RLS is what guards data. Never touch RLS without re-checking the policies in the shared Supabase project (the same one Antidote uses).

## File-size cap: 800 lines

**Every `.rs` file in this workspace stays under 800 lines.** This applies to production code AND tests. When approaching the cap, split into a sibling module under the same parent (e.g. `games/klondike.rs` → `games/klondike/{rules,deal,scoring}.rs`). Do not concatenate unrelated logic into a single oversized file just because it "fits."

## Procedural card rendering — no PNGs

Card faces are drawn via `DrawCtx` primitives in `solitaire-core/src/render/card_face.rs`. **No PNGs, no SVG.** Suit glyphs are custom-traced paths defined alongside the renderer. The four suits and the card back are the only sprite-like things in the entire game and they must all live in the renderer module.

If a card visual primitive (e.g. a corner-rounded gradient highlight, a pip layout helper) would be reusable across other games or apps, **add it to agg-gui first** and call it from Solitaire. Otherwise extend `card_face.rs` or `card_back.rs` directly. Don't reach for raster assets to dodge a hard rendering problem.

## GameRules trait is the seam

All four variants implement `solitaire_core::games::GameRules`. The `GameWidget<R>` widget is generic over `R`; new variants are added by:

1. Creating a new module under `games/` that implements `GameRules`.
2. Adding a button on `ui/title_widget.rs` that constructs the new `GameWidget<NewVariant>`.
3. Adding a row to the `games` table (one of the migrations under `db/migrations/`).

**Do not fork `GameWidget`** to host a new variant. If a variant needs behavior the trait can't express, extend the trait — don't bypass it.

## Drag is owned by GameWidget, not by piles

Pile widgets do not exist. `GameWidget` is the only `Widget` that receives playfield input. It calls `Pile::topmost_under(p)` directly for hit-testing and owns the drag state machine across the entire playfield. **Do not introduce a per-pile child widget** — agg-gui has no pointer-capture API, so a drag that crosses pile boundaries would lose tracking the moment it leaves the originating pile's bounds.

`render::pile_paint::paint_pile(ctx, pile, drag_state, anims)` is a free function called by `GameWidget::paint`. That's the entire pile-rendering path.

## Local development uses agg-gui as a path dep — improve it as you go

When developing on a workstation with the rust-apps superproject checked out (with the agg-gui submodule beside solitaire at `../agg-gui/`), the `[patch.crates-io]` section in the workspace `Cargo.toml` redirects `agg-gui` to the local checkout. **This is the default state** — every commit assumes contributors are running with the path override active.

That means: when solitaire needs an agg-gui feature that doesn't exist yet (a new draw-primitive, an animation easing helper, a card-shaped clipping path), the right move is to **add it to agg-gui itself** (in `C:\Development\rust-apps\agg-gui\agg-gui\src\…`), not to work around it inside solitaire. agg-gui is being grown specifically to support games well; solitaire and antidote are the first real callers driving that growth.

Workflow:
1. Make the change in `../agg-gui/agg-gui/src/…`.
2. Run solitaire against the patched local crate (`cargo check --workspace` — Cargo picks up `../agg-gui/agg-gui` via the patch).
3. When the agg-gui changes are stable, publish a new agg-gui version to crates.io (Lars handles this manually).
4. CI continues building against the published crates.io version because the CI workflow clones `larsbrubaker/agg-gui` as a sibling so the `path = "../agg-gui/agg-gui"` patch resolves there too.

If you're checking out solitaire standalone (no rust-apps superproject), clone agg-gui sibling: `git clone https://github.com/larsbrubaker/agg-gui.git ../agg-gui` from this repo's root.

## Schema is multi-game

The Supabase project is shared with Antidote and any future games. Every user-facing table is keyed `(user_id, game_id)`. The `games` table is the single source of truth for `game_id`. New game variants add a row to `games`; nothing else in the schema changes.

Solitaire game slugs in `public.games`:
- `klondike`
- `freecell`
- `spider`
- `classic`

## Reference implementations

The repo bundles two reference implementations for reading only:

- `reference/avaloniaui-solitaire/` — C# / Avalonia source for Klondike, FreeCell, Spider. Game-rules logic in `Solitaire/ViewModels/`.
- `reference/dualbrain-solitaire/` — VB.NET / WinForms source for the Classic variant.

Treat both as **read-only documentation**: never modify, never include in builds, never commit. Both directories are gitignored.

## Build & test

```bash
cargo check --workspace                    # native targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings

# WASM
wasm-pack build solitaire-wasm --target web --out-dir ../demo/public/pkg --no-typescript
```

`default-members` excludes `solitaire-wasm` so plain `cargo build` doesn't drag wasm-only deps into a native build.

## Plan

Top-level plan and milestones live at `C:\Users\LarsBrubaker\.claude\plans\i-want-to-create-sunny-wigderson.md` (host machine). Don't duplicate it here — it's the source of truth.
