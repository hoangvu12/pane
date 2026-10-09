Landed on `main` in PR #274 (merge commit `bff03cdd`) as `a8a1e67c6ea4bd69dabe515b7040e4a650a4131e`, with the review fixes in `3da35f36`.

Results, per acceptance criterion (all tests ran green in the verify run [37898374301](https://github.com/pane-app/pane/actions/runs/37898374301); its only red is the known macOS pasteboard flake #231):

- With nothing in flight the `Runtime::timers` hook reports both the ticker and the watchdog waiting, with no ticks or looks over 500 ms, and both wake as a call starts — `deadlines.rs` unit tests and `crates/pane-core/tests/runtime_timers.rs` (`idle: both waiting`, `opening the command wakes both`, `at least 10 ticks and 2 looks while "Save after waiting" is in flight`, `waiting again after the answer`).
- The existing suites stayed green unchanged: the full verify run's six Linux and Windows shards cover pausing, stopping, unresponsive calls, `COMPUTE_LIMIT`, a crashed runtime thread replaced, schedules and services — no existing test file was changed for them.
- The race test — `a_call_starting_as_the_ticker_goes_to_wait_is_still_stopped_within_its_limit` — 60 spinning calls under a 30 ms limit with `Yield(1)` epochs, even rounds starting the moment the hook shows the ticker waiting, all ended with the compute-limit error. It passed, including on Windows (the shard where the earlier integration had hung a different test for ~3 h; that test now passes in about 3 s, [37897140611](https://github.com/pane-app/pane/actions/runs/37897140611)).
- `docs/pausing.md` and `docs/generations.md` say the threads sleep while no call runs (and what counts as in flight).
- CI: quick tier [37896410002](https://github.com/pane-app/pane/actions/runs/37896410002) and the verify run above, green on Linux and Windows.

The unticked row is the #189 measurement showing the two threads' wake-ups near zero in `hidden-idle`: no numbers exist yet. The Linux run happens in main's release matrix's smoke job after this merge; the Windows run and any user's-machine numbers need the user's consent. The wake-up mechanism itself is verified by the timers' tests above (no ticks and no looks while idle).

One caveat kept from the review: the thread's own `DropStopped` nudge runs no guest code and is not counted in flight, so a stopped runtime's last nudge does not wake the timers.
