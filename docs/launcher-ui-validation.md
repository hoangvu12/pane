# Launcher UI implementation and validation

Work follows [specification #61](https://github.com/hoangvu12/pane/issues/61).
Application baseline: `748d71e`. Implementation branch: `feat/launcher-ui-spec-61`.

## Integration order

| Ticket | Scope | Prerequisites |
| --- | --- | --- |
| [#62](https://github.com/hoangvu12/pane/issues/62) | Preserve prototype evidence and separate presentation ownership | None |
| [#63](https://github.com/hoangvu12/pane/issues/63) | Reproducible GPUI CE fork and Windows alpha regression | None |
| [#64](https://github.com/hoangvu12/pane/issues/64) | Shared dark/light visuals and responsive layouts | #62 |
| [#65](https://github.com/hoangvu12/pane/issues/65) | Windows native material and validation | #63, #64 |
| [#66](https://github.com/hoangvu12/pane/issues/66) | macOS native material and validation | #63, #64 |
| [#67](https://github.com/hoangvu12/pane/issues/67) | Linux opaque appearance and validation | #63, #64 |
| [#68](https://github.com/hoangvu12/pane/issues/68) | Combined revision validation | #65, #66, #67 |

## Evidence rules

Application behavior tests, renderer output regressions, and native compositor
captures establish different properties. A headless test or successful build
does not establish desktop blur, native input-method operation, or assistive
technology support. A background luminance response alone proves transparency,
not blur; compositor evidence must include softened external edges.

Record the tested application and fork revisions, OS/build, display backend,
scaling, requested appearance, relevant transparency settings and foreground
conditions with each native result. Preserve prototype results separately from
production validation. Do not alter global transparency settings automatically.

## Environment availability

At implementation start on 2026-10-02, the connected development device is
Windows. No macOS or Linux desktop is connected; WSL is not installed. Existing
CI includes Windows, macOS and Linux jobs, but CI build/test results do not replace
the required native material and interaction evidence. Unavailable cases remain
not run and keep final validation open.

The user subsequently scoped execution to Windows first, with a final CI run on
every OS after integration. Native macOS/Linux validation is deferred; it is not
a prerequisite for completing this Windows-focused implementation pass.

## Current result

Implementation and validation are in progress.

### Presentation prefactor (#62)

Integrated as `038a7a8`, with implementation `1254dc6` and preserved prototype
evidence `43aec59`. See [presentation ownership](launcher-presentation.md) and
[prototype recovery and provenance](evidence/ui-prototype/README.md).

- Windows: 49 launcher-window tests and one command-search test pass; checking
  test targets and formatting also pass. The first parallel full test build
  exhausted host memory; scoped serial builds succeeded.
- Native Windows at 96 DPI: startup, typing `rust`, Enter to open its command,
  Enter to invoke Say hello, and Escape passed. The capture shows
  [the Rust guest's answer](evidence/ui-windows/refactor/action.png).
  [Capture metadata](evidence/ui-windows/refactor/pane-run.json) records OS,
  binary SHA256, confirmed foreground input and cleanup of the spawned process.
- The metadata names `b0793c2`, the pre-amend implementation commit whose binary
  was captured. The final `1254dc6` amendment only normalized a saved evidence
  log's final newline; the application code and binary are the same.
- This check used the original appearance before styling. Its theme/material
  environment values were ignored by that build and are not material evidence.

### Shared appearance (#64)

Integrated as `5ca1f06`, implementation `70fd205`. The Windows application suites
pass 50 window tests plus one command-search test. The additional footer test
covers 380x420 and 640x200 layouts, wrapping, bounded height and both scroll
directions. Formatting and checking test targets pass.

Native Windows 11 25H2 build 26200.8737 at 96 DPI, opaque mode, binary SHA256
`B4EEABBA65B91E29CB7F9D86783306787977280E9D4C0E2448EF51405639E053`:

- [Dark root](evidence/ui-windows/opaque-dark/00-initial-window.png), query,
  command invocation and Escape pass. Guarded native header dragging moves
  the window; narrow resize, restore, deactivation and reactivation pass.
  [Run metadata](evidence/ui-windows/opaque-dark/pane-run.json).
- [Light root](evidence/ui-windows/opaque-light-form/00-initial-window.png),
  form editing, [validation](evidence/ui-windows/opaque-light-form/04-after-TAB-ENTER-window.png)
  and [successful submission](evidence/ui-windows/opaque-light-form/06-after-TAB-ENTER-window.png)
  pass. Choice glyphs, focus caret and error/success text remain readable.
  [Run metadata](evidence/ui-windows/opaque-light-form/pane-run.json).
- At 380x420 outer physical pixels, the
  [error wraps](evidence/ui-windows/opaque-narrow/03-after-DOWN-2-ENTER-window.png)
  and the [selected unavailable row](evidence/ui-windows/opaque-narrow/04-after-DOWN-5-window.png)
  retains its full explanation inside the viewport.
  [Run metadata](evidence/ui-windows/opaque-narrow/pane-run.json).

These captures were visually reviewed and their exact binary hash, target
foreground, un-aborted key input and process cleanup checked. Two earlier
attempts were covered by an existing prototype window; the helper sent no keys,
and neither their images nor their input attempts count as evidence. Positioning
only the spawned window near the display's upper-left corner enabled a guarded
focus click. No original prototype window was closed or moved.

These #64 captures use the old renderer pin in opaque mode. Final glass evidence
must use the combined fork and application revision. Native IME, screen-reader
operation, scaled displays and OS transparency-disabled operation remain not run.
