Landed on `main` in PR #271 (merge commit `2ae9e598`) as `36f87602a680c0286153a199f4f6a669067c1a71`, with the review fixes in `c8737999`.

Results, per acceptance criterion (Windows unit tests run in the verify tier's Windows shards; window tests on every system; all green in [37881048674](https://github.com/pane-app/pane/actions/runs/37881048674), whose only red is the macOS pasteboard flake #231):

- After `TaskbarCreated` the icon is added again with the same GUID, tooltip and callback while the preference is to show it, and nothing while hidden — `tray/windows.rs` `taskbar_created_adds_the_icon_again_while_it_should_show`, `a_hidden_icon_stays_hidden_after_taskbar_created`.
- A refused re-add is reported and retried at the next broadcast, preference unchanged — `a_refused_re_add_is_reported_and_tried_again_at_the_next_broadcast`, `a_show_refused_at_start_is_tried_again_when_the_taskbar_starts`.
- GUID stable per path, different per path, refused GUID falls back to the numeric id with a diagnostic — the GUID is a name-based UUID over the canonicalized program path; vectors pinned against Python's `uuid`/`hashlib`.
- A light taskbar selects the dark-on-light variant and a dark taskbar the other; a theme broadcast swaps the icon in place; Pane's own Appearance is not an input — `SystemUsesLightTheme` (fake registry), not `AppsUseLightTheme`.
- The icon is loaded at the small-icon size for the display's DPI — `the_icon_is_made_at_the_small_icon_size_for_the_displays_dpi` (96–384 DPI, real `GetSystemMetricsForDpi`/`CreateIconFromResourceEx`).
- The `tray.rs` window tests stay green — the `Tray` trait, the menu, left click summoning and the preference's save/rollback are unchanged; the refused-preference test now also covers the re-applied call after a refusal.
- macOS: the status item shows the template image and carries an autosave name — compiled and checked on the verify run's macOS leg (the leg's only failure is the clipboard pasteboard flake #231; the adapter itself compiled and its window tests ran in the Linux/Windows legs' platform-independent paths).
- Linux: no tray entry, the General page's explanation unchanged.

The release-validation phases this ticket planned — restarting Explorer and capturing the icon back, the light/dark/selected template-image captures, autosave and menu-bar restart — are recorded in `docs/evidence/settings-79/native-validation.md` and wait for the native pass (#84). The notification version 4 click mapping and the elevated-Pane message filter are called out there as things only a real run can confirm.
