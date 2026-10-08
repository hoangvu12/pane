# Per-platform resource and latency measurements

Recorded 2026-09-30 for
[#4](https://github.com/hoangvu12/pane/issues/4) (the P3 prerequisite of
[the spec](https://github.com/hoangvu12/pane/issues/1)'s G7, US04, US14,
US81; T02, T24, T25). This is the record the platform pages link to for
resource and latency evidence and targets. Its status is honest and narrow:
the workload, the sampler, the targets file and the CI wiring exist, and
**no number has been measured yet** — the machine that wrote them has no
display, so the workload cannot run there, and CI's Linux leg has not run
it at the time of writing. Every target below is proposed in shape only,
pending the first collected record and the user's confirmation. No target
is inferred from the older CLI peaks ([the QuickJS spike's
measurements](qjs-p3-port-spike/README.md) stay what they are: same-workload
CLI comparisons, not launcher budgets); Q3's numerical budgets remain open
until the numbers exist and are accepted.

[#189](https://github.com/hoangvu12/pane/issues/189) (the measurement of
[#188](https://github.com/hoangvu12/pane/issues/188), idle cost) added the
`hidden-idle` phase, the wake-ups of every thread by its name, and a Windows
script for the hidden phase. Its baseline is not recorded yet either (see
[Evidence](#evidence)).

## The workload

One documented workload, run by
[`scripts/measure-linux.sh`](../../scripts/measure-linux.sh) under Xvfb on
the same runner, with the same guests and the same Pane binary as the smoke
([the Linux baseline](../platforms/linux.md)). It requires what
`scripts/smoke-linux.sh` requires (Xvfb, xdotool, Pillow, a Vulkan driver)
plus the guests built (`cargo xtask ci`) and the default extensions'
payloads in `target/dist/artifacts` (`cargo xtask package-linux --dev`,
which CI's smoke, run just before the workload in the same job, leaves
there). The phases are fixed; their durations are the defaults (each has a
`PANE_MEASURE_*` environment override for a quick run, recorded in
`record.json`):

| Phase | What happens | What it measures |
| --- | --- | --- |
| `cold-start` | Pane starts with a fresh data folder, nothing installed (screenshot 1, the smoke's hint-line check) | cold-start latency (exec → window appears), startup RSS/CPU |
| `warm-start` | three restarts with the same data folder | warm-start latency (median of three) |
| `idle-core` | Pane sits at root search, untouched, 60 s | idle whole-tree RSS and CPU, and the wake-ups of each thread |
| `hidden-setup` | a data folder of its own: Pane starts (its one show), is hidden with Escape at its blank root search, acquires its five default extensions from the payloads served on 127.0.0.1, and settles for 30 s (`PANE_MEASURE_SETTLE_SECONDS`) | nothing with a target: the setup the next phase follows |
| `hidden-idle` | Pane, hidden, with its default extensions and nothing else installed, untouched, 60 s (`PANE_MEASURE_HIDDEN_SECONDS`); the workload fails if the launcher shows itself | Pane's cost while hidden: whole-tree RSS and CPU share, and the wake-ups of each thread, the epoch ticker's and the watchdog's among them (#188) |
| `installed-unused` | the seven packages calculator, quicklinks, applications, files, sample-rust, sample-js, sample-ts are installed, none invoked, 60 s | the cost of installed-but-unused extensions (T02); the applications extension's host-side scan of desktop entries runs, as it does for a user's Pane |
| `calculator` | the calculator, a default extension: an expression typed into root search, Enter copies the answer, Escape clears, ten times (the first answer is color-checked) | active-command use; repeated growth is the leak bound |
| `continuing-work` | the Watching continuing service ([services](../services.md), a cycle a second) and the Counting scheduled work ([schedules](../schedules.md), a run a minute) run while Pane sits at root search, 90 s | steady-state background work; their counts in the packages' content prove the work ran (asserted in the script), and screenshots 5 and 6 show both commands |
| `reload` | one package of its own in a fresh data folder: its component is swapped between the Rust and the JavaScript sample and reloaded six times, each reload waited for | resource release across repeated reload (T24) |
| `disable` | the same package disabled and enabled again five times, each state change waited for | resource release across repeated disable |

In the hidden phases, Files' index covers an empty folder of the
workload's (`PANE_TEST_FILE_INDEX_HOME`, which development builds read), not
the runner's home, so `hidden-idle` measures Pane idling rather than a first
walk of a home folder; its watcher still runs. Whatever still runs after the
settling, such as the applications' icon extraction after a start, is part
of `hidden-idle`, and the per-thread counts say whose it is.

`record.json` records the machine and the build settings: the OS
(`/etc/os-release`), the kernel and architecture (`uname -srm`), the
commit, the Rust version, the Pane binary's path, size and profile, and the
workload's parameters. The default binary is the smoke's `target/debug/pane`:
a release build's first setup reaches for the not-yet-deployed artifact
source at start, so the release profile's workload waits for that source;
pass a release binary to the script when that day comes and the profile is
recorded either way.

## The record

The output folder holds, beside the workload's screenshots:

- `samples.jsonl` — one JSON line per sample (1 s cadence), each holding
  the phase, the root pid, and every process of Pane's whole tree (Pane
  and any helper it started) with its pid, parent, name, resident set size
  and CPU ticks, and every thread of each process with its id, name,
  voluntary and involuntary context switches and CPU ticks, read from
  `/proc` ([`scripts/proc_tree.py`](../../scripts/proc_tree.py) `watch`);
- `events.jsonl` — the latencies and counts the workload itself measured
  (window latencies, how many service cycles and scheduled runs happened);
- `summary.json` — the per-phase aggregates (`summary` mode): RSS
  min/median/p95/max, CPU seconds and share, growth from the phase's first
  to last sample, the wake-ups (below), the window latencies, and the
  derived per-extension cost, with `record.json` embedded as
  `environment`;
- `system.txt`, `stderr.log`, `xvfb.log` and `control` — the environment
  and what ran.

`proc_tree.py selfcheck` checks the sampling logic without a display
against a `sleep` process with children of its own (both tree-walk paths,
the summary's window/extra merging, the threads counted by name, the
wake-up arithmetic on hand-made samples, and the check's
pending/breach/unmeasured behaviour); the workload runs it first. Sampling
is Linux only: `/proc` is where it reads. The Windows script writes samples
of the same shape itself (below); `summary` and `check` read either.

### Wake-ups by thread

Each phase's `wakeups` in `summary.json` counts how often the tree's threads
were woken over the phase:

```json
"wakeups": {
  "counted": "voluntary context switches",
  "total": 6600,
  "per_second": 110.0,
  "by_thread": {
    "pane-runtime-ep": { "threads": 1, "count": 6000, "per_second": 100.0, "cpu_seconds": 0.04 },
    "pane-runtime-wa": { "threads": 1, "count": 600, "per_second": 10.0, "cpu_seconds": 0.01 }
  }
}
```

(The numbers show the shape only; nothing is measured yet.) On Linux the
count is a thread's voluntary context switches, `voluntary_ctxt_switches`
in `/proc/<pid>/task/<tid>/status`: each is the thread blocking (a sleep, a
lock, a read) and later being woken. A thread there at the phase's first
sample counts what it did since; a thread started during the phase counts
everything it did; threads of one name are added up, and `threads` says how
many there were. `cpu_seconds` is their CPU time over the phase, so a
thread that wakes rarely but computes is seen too.

The name is the thread's own: `std::thread::Builder::name` in Pane's code.
Linux keeps at most 15 bytes of it (the kernel's `comm`; Rust's standard
library cuts a longer name there), so a runtime thread's epoch ticker,
named `pane-runtime-epoch` (`crates/pane-core/src/runtime/deadlines.rs`,
`tick`), is listed as `pane-runtime-ep`, and its watchdog,
`pane-runtime-watchdog` (`runtime/supervisor.rs`, `watchdog`), as
`pane-runtime-wa`. Names that share their first 15 bytes are added up
together: `pane-extension-runtime` (the runtime thread itself) and
`pane-extension-check` both as `pane-extension-`, and
`pane-applications`, `pane-application-icons` and
`pane-application-changes` all as `pane-applicatio`. A thread nobody named
carries the process's name (`pane`). On Windows, names are whole.

## The Windows script

[`scripts/measure-windows.ps1`](../../scripts/measure-windows.ps1) measures
the hidden phases on Windows, for a scratch Pane on the machine it runs on.
It is not part of CI: it runs on the user's own machine, **only when the
user says the machine is free** (it is also used for games), and the
numbers it records then go under [Evidence](#evidence).

```powershell
cargo build -p pane
cargo xtask package-windows --dev   # the default extensions' payloads, in target/dist/artifacts
./scripts/measure-windows.ps1 -OutDir measure-windows
```

It starts the development build (`-Binary` names another) with
`PANE_DATA_DIR` and `LOCALAPPDATA` (where Pane keeps its cache) pointing
into a new folder of the temporary folder, for that one process only. The
scratch Pane acquires its five default extensions there from
`target/dist/artifacts`, served on 127.0.0.1 by
[`scripts/artifact_server.py`](../../scripts/artifact_server.py) as the
smoke serves them, and Files' index covers an empty folder there. The
launcher's window shows once, as every start shows it, and is hidden at
once with Escape at its blank root search, posted to Pane's own window
(typed only if that did not hide it and Pane's window is the foreground
window). Then the script samples Pane's tree every second: `hidden-setup`
until the defaults are recorded and 30 s more (`-SettleSeconds`), then
`hidden-idle` for 60 s (`-HiddenSeconds`). Each sample reads the system's
process records once (`NtQuerySystemInformation`, `SystemProcessInformation`):
each process's CPU time and working set, and each thread's context switches
and CPU time, named by the thread's own name (`GetThreadDescription`; Rust's
standard library gives every named thread one). The samples have
`proc_tree.py`'s shape, and `proc_tree.py summary` turns them into the same
`summary.json`, with `record.json` (the Windows version and build, the
binary's path, size and SHA-256, the commit, the parameters) embedded.
There is no Windows targets file yet, so no `check` runs.

What it leaves alone: it never captures the screen and never brings Pane
forward. It never reads or writes the user's own Pane data or cache. A
scratch Pane reconciles the launch-at-login choice of its own settings with
the per-user startup list's `Pane` value at start, and would remove a value
it did not choose; so the script writes the scratch settings first with the
choice the list already holds, and checks the value afterwards, putting it
back if anything changed it. It quits only the Pane it started (closing its
window, then stopping it if it does not quit), and removes the scratch
folder, which holds what Clipboard History recorded during the run
(`-KeepScratch` keeps it). While it runs, the scratch Pane shows its tray
icon and takes the default hotkeys if no other Pane holds them.

On Windows a thread's count is every context switch to it
(`SYSTEM_THREAD_INFORMATION.ContextSwitches`), voluntary or not; for a
thread that mostly sleeps, that is its wake-ups. `rss_kb` is the working
set. `counted` in `wakeups` says which count a summary holds.

## The targets

Proposed, **every number pending**; the machine-readable file is
[`scripts/resource-targets.json`](../../scripts/resource-targets.json),
where a null ceiling means pending. The summary is checked against it at
the end of every workload run (`proc_tree.py check`): a confirmed ceiling
that the summary exceeds fails the run; a pending one is reported and
passes.

| Metric | `summary.json` field | Target | Status |
| --- | --- | --- | --- |
| Cold-start latency (exec → window) | `cold-start.window_ms.median` | ceiling | pending a measurement |
| Warm-start latency (median of three) | `warm-start.window_ms.median` | ceiling | pending a measurement |
| Idle whole-tree RSS | `idle-core.rss_kb.max` | ceiling | pending a measurement |
| Idle whole-tree CPU share | `idle-core.cpu_share` | ceiling | pending a measurement |
| Idle wake-ups a second (every thread of the tree) | `idle-core.wakeups.per_second` | ceiling | pending a measurement |
| Hidden whole-tree RSS (default extensions) | `hidden-idle.rss_kb.max` | ceiling | pending a measurement |
| Hidden whole-tree CPU share | `hidden-idle.cpu_share` | ceiling | pending a measurement |
| Hidden wake-ups a second (every thread of the tree) | `hidden-idle.wakeups.per_second` | ceiling | pending a measurement |
| Additive cost per installed-but-unused extension | `installed-unused.per_extension_kb` | ceiling | pending a measurement |
| Installed-but-unused RSS | `installed-unused.rss_kb.max` | ceiling | pending a measurement |
| Active-command RSS (calculator, repeated) | `calculator.rss_kb.max` | ceiling | pending a measurement |
| Active-command growth (leak bound) | `calculator.growth_kb` | ceiling | pending a measurement |
| Continuing-work RSS (service + schedule) | `continuing-work.rss_kb.max` | ceiling | pending a measurement |
| Continuing-work growth | `continuing-work.growth_kb` | ceiling | pending a measurement |
| Resource release across repeated reload | `reload.growth_kb` | ceiling | pending a measurement |
| Resource release across repeated disable | `disable.growth_kb` | ceiling | pending a measurement |

How a number becomes a target: the workload runs on CI's Linux leg
(wired in [ci.yml](../../.github/workflows/ci.yml), the record uploaded as
the `resource-measurements` artifact); the measured values are recorded
here as this machine's data point; a ceiling is proposed from them with
headroom and a stated rationale; **the user confirms it** (Q3's budgets
stay open until then); the number lands in `scripts/resource-targets.json`
and the check enforces it from then on. A breach is narrow corrective work
on the named metric, never an open-ended optimization ticket. Workload
integrity (that the service actually cycled, that the schedule actually
ran, that each reload actually replaced the managed copy) is asserted by
the workload itself, not by the targets.

## Evidence

### Linux, CI (x86_64, Xvfb)

Pending the next run of the branch: the numbers land in the
`resource-measurements` artifact (`measure/summary.json`) and are recorded
here once a run collects them. The runner is the `ubuntu-24.04` leg the
[Linux baseline](../platforms/linux.md) documents.

The `hidden-idle` baseline on `main` (#189), with the wake-ups of
`pane-runtime-ep` and `pane-runtime-wa`, is pending the same: a run of the
release matrix's smoke job on Linux, or the workload on demand.

### Linux, this machine (aarch64, no display)

Ubuntu 24.04.5 LTS, kernel 5.15.0-1081-oracle, aarch64, 3 CPUs, no
display and no Xvfb: the GUI workload cannot run here, so no aarch64
numbers exist. What ran here: `proc_tree.py selfcheck` (a `sleep` process
with children, both tree-walk paths, the summary and check behaviour), the
whole record path with a synthetic record (`summary` and `check`, including
the per-extension derivation and a deliberate breach failing), a full
stubbed dry run of the workload's mechanics end to end (every phase, the
sampler wiring, the record and the check; no GUI, so no measurement), and
`bash -n` on both scripts.
aarch64 GUI numbers would need the smoke's environment on an aarch64
machine; only the x86_64 CI leg is claimed.

### Windows, the user's machine

[The Windows script](#the-windows-script) exists for the hidden phases;
nothing is measured yet. Its baseline on `main` is taken on the user's
machine only when the user says the machine is free, and recorded here
with `record.json`'s machine details. The other phases have no Windows
sampler or workload yet.

### macOS, and the rest of Windows

Nothing measured, nothing wired for macOS: `/proc` is Linux-only, so its
process-tree sampler does not exist yet, and Windows has only the hidden
phases. When those legs measure, each should run the same phases with the
same metrics and its own targets file: a tree sampler (macOS: the tree of
pids and their RSS/CPU through `ps`/`proc_pidinfo`; Windows: the script's
sampler, or a Job Object if the tree is to be contained), the
window-appear latency for cold and warm start, the same
idle/installed/active/continuing/lifecycle phases, and the same record
shape so the platform pages stay comparable. This is recorded as future
work, not silently claimed; G7 needs each claimed combination's own native
results.

## Release validation

[The release checklist](https://github.com/hoangvu12/pane/issues/57)'s row
"Run the documented P3 resource workload on this platform, measuring the
full process tree and comparing with its recorded targets" runs, on Linux:

```sh
bash scripts/measure-linux.sh measure
python3 scripts/proc_tree.py check measure/summary.json scripts/resource-targets.json
```

CI's Linux leg runs both on every push; the release validation rechecks
the candidate's own run and record. The Windows and macOS rows stay open
until those legs measure (above). Recorded here because this ticket does
not edit the issue.

## Known limits

- The measured combination is Xvfb with software Vulkan (lavapipe), the
  [Linux baseline](../platforms/linux.md)'s — not a desktop session, not a
  real GPU, and the numbers say so in `record.json`.
- The default binary is the development profile (the smoke's); the release
  profile's numbers wait for the deployed artifact source (above).
- A 1 s cadence over 60–90 s phases catches steady state and gross growth,
  not sub-second spikes. Window latencies are wall-clock to the window's
  appearance, polled at 0.2 s, so they quantize by about that much and are
  an X11-under-Xvfb property, not a desktop-session perception number.
- One runner is one data point; variance across runners is unknown until
  several runs are collected, and CPU share can be inflated by other work
  on the runner.
- Warm start measures a restart of a Pane with nothing installed; a warm
  start with nine packages installed is part of the `installed-unused` and
  `continuing-work` phases' start, recorded in their samples.
- `hidden-idle` follows a first setup: the defaults were acquired in the
  same run, so what a start does once (the applications' icons) may still
  run after the settling, and is counted there by thread. Its development
  build may also offer an update to Pane itself, when the served payloads
  name a newer package (CI's smoke leaves one); nothing is downloaded.
- Wake-ups are counted at the sample cadence: a thread that ends between
  two samples loses the switches since its last one. On Linux, names that
  share their first 15 bytes are one entry (above).
