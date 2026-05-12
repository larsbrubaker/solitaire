# Solitaire — Claude Code guide

Rules and gotchas Claude can't infer from the code. Architecture itself (workspace layout, trait seams, module structure) is derivable by reading the source — don't restate it here.

## Build & test

```bash
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings

# WASM. The wasm crate is excluded from default-members so plain
# `cargo build` doesn't drag wasm-only deps into the native build.
wasm-pack build solitaire-wasm --target web --out-dir ../demo/public/pkg --no-typescript
```

## Rules

**`solitaire-core` MUST stay `wasm32`-clean.** No `tokio`, `dotenvy`, `dirs`, `winit`, or `wgpu`. Native-only services are injected through traits in `solitaire_core::platform`.

**Platform crates contain no game or UI content.** `solitaire-native` and `solitaire-wasm` are window/canvas + event-loop + persistence shells. Every widget, menu, screen, rule, and dialog lives in `solitaire-core`. Platform code calls shared builders like `solitaire_core::ui::build_solitaire_app()` and forwards events.

**Every `.rs` file stays under 800 lines** (production AND tests). When approaching the cap, split into a sibling module under the same parent (e.g. `games/klondike.rs` → `games/klondike/{rules,deal,scoring}.rs`). Don't concatenate unrelated logic into one oversized file because it fits.

**All widget code is Y-up first-quadrant.** The OS event Y-flip happens once inside `App::on_mouse_*`. Keep hit-testing and card-layout math in Y-up.

**Card art lives in `solitaire-core/src/render/`.** The active deck is the bundled CC0 SVG at `solitaire-core/assets/cards/english_pattern_cc0.svg` (Wikimedia Commons, "English pattern playing cards deck PLUS CC0", public domain). [`render/svg_deck.rs`](solitaire-core/src/render/svg_deck.rs) parses + rasterizes the master once per atlas rebuild and slices out 52 faces + 1 back. `card_face.rs` / `card_back.rs` retain procedural fallbacks but aren't currently wired into the atlas. Any new deck theme should drop in here too — pick whatever rendering approach (SVG, procedural `DrawCtx` paths, or a mix) gives clean scale-up. Bundled decks should be CC0 / public-domain so we can redistribute freely.

**Icons render through Font Awesome glyphs.** Bundle the FA TTF as `include_bytes!`, load it as an `agg_gui::text::Font`, and draw the Unicode code point with `ctx.fill_text`. Code-point constants live in `ui/icons.rs` (e.g. `FA_GEAR = '\u{f013}'`); don't sprinkle raw `'\u{...}'` literals through widget code. agg-gui's glyph cache handles per-glyph caching, so don't pre-rasterize.

**Don't fork `GameWidget`.** New variants implement `solitaire_core::games::GameRules`. If a variant needs behavior the trait can't express, extend the trait — don't bypass it.

**Pile widgets DO NOT EXIST.** agg-gui has no pointer-capture API, so a drag that crossed pile boundaries would lose tracking. `GameWidget` owns drag state for the entire playfield and hit-tests piles directly via `Pile::topmost_under(p)`. Pile rendering is the free function `render::pile_paint::paint_pile(...)` called from `GameWidget::paint`.

**Mouse-wheel `delta_y` convention is `winit` / `WheelEvent`'s.** Positive = user wants to see content ABOVE (wheel rotated forward, post-OS-natural-scroll). Scroll containers DECREASE their offset on positive `delta_y`. The native shell forwards winit's `MouseScrollDelta` to `App::on_mouse_wheel_xy_mods` as-is — no sign flip. Don't add scrolling to a widget that doesn't go through agg-gui's `ScrollView`/`TreeView`/`MarkdownView`; if you do, the system convention is whatever they implement.

## agg-gui is a path dep — extend it as you go

`[patch.crates-io]` in the workspace `Cargo.toml` redirects `agg-gui = "0.2"` to `../agg-gui/agg-gui`. When Solitaire needs a draw-primitive, easing helper, or clip path that doesn't exist yet, **add it to `../agg-gui/agg-gui/src/…` first** and call it from Solitaire. agg-gui is being grown to support games well; Solitaire is one of the first callers driving that growth. CI clones `larsbrubaker/agg-gui` as a sibling so the patch resolves there too. If you cloned Solitaire standalone:

```bash
git clone https://github.com/larsbrubaker/agg-gui.git ../agg-gui
```

## DB

Access goes through Supabase REST (`/rest/v1/*` PostgREST + `/auth/v1/*`) over `ehttp` — no direct Postgres (wouldn't work in WASM). The anon key ships in the build artifact; **RLS is what guards data**, so don't touch RLS without re-checking policies (the project is shared with Antidote).

Tables are keyed `(user_id, game_id)`. `public.games` is the source of truth for `game_id`. Active slugs: `klondike`, `freecell`, `spider`.

## Reference implementations are read-only

`reference/avaloniaui-solitaire/` (C# Avalonia — Klondike, FreeCell, Spider) and `reference/dualbrain-solitaire/` (VB.NET WinForms — Microsoft Classic) are bundled as documentation. **Never modify, build, or commit them.** Both directories are gitignored.

## Plan

Active milestone plan: `~/.claude/plans/i-want-to-create-sunny-wigderson.md` (host machine, not in repo).
