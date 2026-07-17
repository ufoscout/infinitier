# Infinitier

## GUI verification

Drive the running Keeper through the **egui MCP** — see [docs/egui_mcp.md](docs/egui_mcp.md).
Run the app with `EGUI_INSPECTION=1` and `attach`; then `query_tree` for widget ids/values and
act on them by id.

Prefer this over `xdotool` + screenshot coordinate-hunting: it doesn't steal window focus, and
you can assert widget values directly instead of reading pixels.
