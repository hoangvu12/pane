## Parent

https://github.com/pane-app/pane/issues/127 (Extension updates)

## What to build

Each update pass — automatic or asked for — records its **update results** as Pane's own record beside `updates.json` (not extension data): for each package considered, its identity, title, group and detail. **Updated**: the old and new version (or commit). **Skipped**: the reason (pinned, a local or development copy, automatic updates off, disabled, paused, needs a newer Pane, not available on this system). **Failed**: the explanation the status line gives today, ending "It keeps running its installed code", or the startup failure that paused it. The record of the latest pass that changed or failed anything is kept across restarts; a pass that found nothing new does not replace it.

A **results view** — a Pane screen opened from View Details and from the Extensions group in Settings — lists the groups in the order Updated, Skipped, Failed, hiding empty ones, each row with the extension's icon, title, detail and a status tag, and a search field, working like any list (search, arrows, Enter, the Actions panel). Row actions: Show Extension (its page in Settings) and Copy Details.

**Background passes** stay silent when everything succeeds or is skipped. If any package failed, Pane announces it once, the next time the launcher is shown, with a failure toast ("1 extension update failed") carrying View Details; the announcement is not repeated for the same failure. The status-line messages of the existing single background updates are replaced by these results and the announcement.

The first automatic check moves to one minute after start (today it is one second), so the check never competes with Pane's own start; the launcher's clock seam keeps tests deterministic.

## Acceptance criteria

- [ ] A pass's groups, details and reasons are recorded and shown in the view, empty groups hidden, in the order Updated, Skipped, Failed
- [ ] Updated rows show the old and new version (or commit); Skipped rows say why; Failed rows say what failed and that the extension keeps running its installed version
- [ ] The record persists across restarts and is replaced only by a pass that changed or failed something
- [ ] A background failure is announced once the next time the launcher is shown, with View Details; successes and skips stay quiet
- [ ] The results view works by keyboard: search, arrows, Enter and the Actions panel; Show Extension and Copy Details work
- [ ] The first automatic check runs one minute after start, through the launcher's clock
- [ ] Launcher public-interface tests against controlled loopback sources cover the groups, reasons, persistence and replacement rules, and the announcement
