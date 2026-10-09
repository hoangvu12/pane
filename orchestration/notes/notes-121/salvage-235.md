# Salvage from the interrupted #235 run

A previous #235 implementer run was killed by an engine restart mid-work. Its
uncommitted work — never pushed, so never CI-checked, and written against the
pre-#271..#275 base — is preserved in:

- `salvage-235.patch` — the full diff (WIT ×2, runtime.rs, launcher.rs,
  launching.rs, packages.rs, and the new designed.rs)
- `salvage-235-designed.rs` — the new file standalone (785 lines, includes
  its own tests)

`launcher.rs` and `runtime.rs` were moved by the 30 commits that landed on
main overnight, so the edits to them in the patch will NOT apply cleanly.
`designed.rs` is a new file and applies to any base. Treat the patch as a
design reference and a starting point for code, not as something to
`git apply` wholesale: re-derive by hand on the current base, with CI feedback.

What the interrupted run had designed (its decisions, worth adopting or
consciously overriding — all unvalidated):

1. **WIT envelope** (`wit/extension.wit` + `guests/pane-extension/wit/`, ~83
   lines each): `record rendered { tree, refresh-after-ms: option<u32> }`,
   `record ui-event { render: u64, key: string, callback: u32, payload: string }`,
   `record outcome { push/replace/pop }` (fields documented as inert until
   #239), `resource view { render(context: string) -> result<rendered, string>;
   handle-event(ui-event) -> result<outcome, string> }`, and
   `open-designed-view(command, launch)` — resolving the collision with the
   existing `open-view(item-id) -> custom-view` (kept until #242).
2. **pane.json mode** `"designed"` for commands whose screen is the designed
   tree (today's default `render` list path unchanged). Check ADR 0037
   (`docs/adr/0037-*.md`): it declares modes `view`/`no-view` and "a view
   command renders its UI on the tree" — decide whether `designed` is a
   transitional third value (migrating later when #240/#241 move List and
   Form onto the tree) or whether another shape fits better. Document the
   choice.
3. **Strict parser** `crates/pane-core/src/runtime/designed.rs`:
   `COMPONENT_SET = (1, 0)`; limits `MAX_NODES = 10_000`, `MAX_DEPTH = 64`,
   `MAX_TREE_BYTES = 4 MiB`, `MAX_TEXT_CHARS = 64 KiB`; `DesignedTree`/`Node`/
   `NodeKind`; unknown-node → `fallback` else children; `requires` honoured;
   wrong/missing fields → `CallError::Unreadable` (the command's failure,
   never a crash); over-limit or wrong major → the extension's error, last
   good tree kept. JSON kept at the runtime edge; the launcher and window see
   only the typed tree.
4. **Runtime plumbing** (runtime.rs, +293): `DesignedView`, `DesignedEvent`,
   `render_context(render: u64)`, `open_designed_view`,
   `designed_view_event`, `render_designed`.
5. **Launcher/packages** (launcher.rs +175, launching.rs +7, packages.rs +11)
   — opening and driving the designed view, mode parsing.

The planned doc name was `docs/designed-tree.md` (parallel to
`docs/list-tree.md`).
