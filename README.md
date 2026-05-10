# Solitaire

Four solitaire variants in Rust — rendered with [agg-gui](https://github.com/larsbrubaker/agg-gui), persisted to Supabase, runs natively (winit + wgpu) and in the browser (WebAssembly).

The four games:

- **Klondike** — classic 7-tableau / 4-foundation / 1-stock / 1-waste solitaire (1-card draw to start; 3-card draw later).
- **FreeCell** — 8 cascades / 4 free cells / 4 foundations. No stock; multi-card moves gated by free-cell count.
- **Spider** — 10 cascades / 8 foundations. 1-suit / 2-suit / 4-suit difficulty selector.
- **Classic** — Microsoft-style Klondike variant ported from the [DualBrain VB.NET](https://github.com/DualBrain/Solitaire) original.

The 3-game shell (Klondike, FreeCell, Spider) follows the [AvaloniaUI/Solitaire](https://github.com/AvaloniaUI/Solitaire) C# layout; **Classic** is a 4th option added on top.

## Quick start

```bash
# Native
cp .env.example .env          # fill in SUPABASE_ANON_KEY
cargo run -p solitaire-native

# WebAssembly
wasm-pack build solitaire-wasm --target web --out-dir ../demo/public/pkg --no-typescript
cd demo && bun install && bun run dev
```

## Workspace layout

```
solitaire-core/    # game logic, widgets, Supabase REST client (target-agnostic)
solitaire-native/  # winit + wgpu shell
solitaire-wasm/    # cdylib wasm-bindgen shell
demo/              # TypeScript bundling shell for the WASM build
reference/         # AvaloniaUI + DualBrain reference repos (read-only; gitignored)
```

`solitaire-core` is `wasm32`-clean — no `tokio`, no `dotenvy`. Both shells inject `Storage` impls.

Card faces are **drawn procedurally** through agg-gui's `DrawCtx` — no PNGs, no SVG. See `solitaire-core/src/render/card_face.rs` once Phase 1 lands.

## Database

Schema is shared with [Antidote](https://github.com/larsbrubaker/antidote): `games`, `user_scores (user_id, game_id) PK`, `user_progress`, `user_settings`. Same Supabase project; new rows in `games` for slugs `klondike`, `freecell`, `spider`, `classic`.

Auth: Supabase email/password via REST. Tokens cached in a JSON file on native, `localStorage` in the browser. RLS enforces `auth.uid() = user_id` on user-scoped tables.

## Status

| Phase | Description | State |
|-------|-------------|-------|
| 0 | Repo scaffold + CI + Pages deploy | scaffolding committed |
| 1 | Card framework (cards, piles, session, render) | pending |
| 2 | Klondike | pending |
| 3 | FreeCell | pending |
| 4 | Spider | pending |
| 5 | Classic | pending |
| 6 | Persistence + leaderboard | pending |

## License

MIT — see [LICENSE](LICENSE).
