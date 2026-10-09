Implements spec #127 (tickets #256, #267; #269 stays blocked by #207):

- **Record update results, show them, and announce background failures (#256)**: every update pass — automatic or asked for — records its per-package outcomes (Updated with old→new version/commit, Skipped with its reason, Failed with the explanation and that the extension keeps running its installed code) in a Pane-owned record beside `updates.json`, kept across restarts when a pass changed or failed anything. A results view lists the groups Updated, Skipped, Failed (empty groups hidden) with search, arrows, Enter and the Actions panel, and Show Extension / Copy Details row actions. Background failures are announced once, the next time the launcher is shown. The first automatic check moves to one minute after start.
- **Check for Extension Updates on demand and update all (#267)**: a Check for Extension Updates command in root search and a Check for updates button (with Last checked) on the Settings Extensions page start one pass that checks every updatable package — including ones whose automatic updates are off, disabled and paused — and applies what it finds, listing pins, local folders and development copies as Skipped with their reasons. Progress and the summary show in the status line until the toast surface can carry them; View Details opens the results view, whose Retry and Update Now row actions act on a single package.

Closes #127
Closes #256
Closes #267

Signed-off-by: hoangvu12 <hggaming91@gmail.com>
