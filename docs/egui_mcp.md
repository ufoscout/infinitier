# Driving the Keeper with the egui MCP

egui 0.35 ships an **inspection port**: eframe opens a TCP socket that an external tool can use
to read the app's **AccessKit widget tree**, **inject input**, and **capture screenshots**. The
[`egui_mcp`](https://crates.io/crates/egui_mcp) server exposes that to an agent as MCP tools.

## One-time setup (already done in this repo)

1. **eframe feature** — `Cargo.toml` enables it:
   ```toml
   eframe = { version = "0.35", features = ["inspection"] }
   ```
   It is inert unless `EGUI_INSPECTION` is set at runtime, so it's production-safe.

2. **The server**:
   ```sh
   cargo install egui_mcp        # installs the `egui-mcp` binary
   ```

3. **Register it with Claude Code** (already in `~/.claude.json` for this project):
   ```sh
   claude mcp add egui egui-mcp
   ```
   > Registering mid-session does **not** expose the tools to that session — they appear on the
   > next run (or after `/mcp` → reconnect).

## Running the app with inspection on

```sh
EGUI_INSPECTION=1 cargo run -p infinitier_keeper -- --savegame 0 --game-path "…"
# binds 127.0.0.1:5719
```
`EGUI_INSPECTION=0.0.0.0:5719` exposes it on the network — **no authentication**, so prefer
loopback. Unset/`0`/`false` = fully off.

Check it's up: `ss -ltn | grep 5719`.

## Using it

Call `attach` first (defaults to `127.0.0.1:5719`), then drive the app.

| tool | what it does |
|---|---|
| `attach` / `disconnect` / `status` | connect to the app's inspection port |
| `query_tree` | find nodes by `role` / `content_contains` / `label_contains` / `value_contains` → returns `id`, `value`, `bounds` |
| `get_node` | read one node by `id` (great for asserting a value) |
| `click` | click a node (by `id`/text/role) or raw `pos`; `count: 2` = double-click, `button: secondary` = right-click |
| `type_text` | type into the focused widget, or focus a node first via a locator (AccessKit focus — doesn't move the caret) |
| `press_key` | e.g. `{"key":"A","modifiers":{"command":true}}` = select-all; `Enter`, `Tab`, `Backspace`, … |
| `hover`, `scroll`, `drag` | the rest of the pointer surface |
| `screenshot` | PNG; `save_path` to write it out |
| `resize`, `wait_for` | viewport size; poll until the tree settles |
| `batch` | several actions in one round trip, e.g. click → type → query_tree |

### Typical loop

```
attach {}
query_tree {"role":"TextInput","limit":20}      → ids + values + bounds
batch {"actions":[
  {"name":"click","args":{"id":"<id>"}},
  {"name":"press_key","args":{"key":"A","modifiers":{"command":true}}},
  {"name":"type_text","args":{"text":"20"}}
]}
get_node {"id":"<id>"}                          → assert the value
```

## Gotchas (learned the hard way)

- **Text injection needs the app window to have OS focus.** Tree reads and clicks work while the
  app is in the background, but injected `type_text` / `press_key` silently do nothing if the
  window isn't focused. Activate it first (`xdotool windowactivate --sync <win>`), and do the
  whole scenario in **one** tool call — anything that steals focus in between (a terminal, a
  permission prompt) breaks the next keystroke.
- **Screenshots need a visible window** — a fully-occluded/minimised window can't render a frame,
  so the call times out.
- **Coordinates are logical points**, matching `bounds` from `query_tree`. `screenshot` defaults to
  `pixels_per_point: 1.0` so its pixels line up 1:1 with those coordinates. (The physical window
  here is 2× — logical 511,268 = physical 1022,536.)
- **`Label` widgets carry their text in `value`, not `label`.** Prefer `content_contains`, which
  matches either.
- **`query_tree` only sees rendered widgets** — scroll/navigate to them first.
- **App stderr is block-buffered when redirected to a file**, so `log::` output can lag behind
  interleaved `echo`s. Don't trust ordering against markers you append yourself; grep the whole
  file.
- `GetTree` forces a repaint, so polling via the MCP changes the frame cadence — keep that in mind
  when chasing timing-sensitive bugs, and cross-check with a plain (non-inspection) build.

## Under the hood

`egui-mcp` speaks the [`egui_inspection`](https://crates.io/crates/egui_inspection) protocol:
a `b"eins"` + u32-version handshake, then `4-byte big-endian length + MessagePack` frames,
strictly request→response (`GetInfo`, `GetTree`, `GetScreenshot`, `ApplyEvents`, `Resize`).
You can speak it directly, but the enum encoding is rmp-serde's — using `egui-mcp` is easier.
