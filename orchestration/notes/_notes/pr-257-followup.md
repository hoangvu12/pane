Two `eprintln!`s from #257 (`pane-ext` and the local channel) were left behind when #271 landed #133's one diagnostic path, so `diagnostics::tests::nothing_else_writes_to_standard_error` fails on `main` (release run [37884589449](https://github.com/pane-app/pane/actions/runs/37884589449), shard 2/3 on all three systems). This routes both through `pane_core::diagnostic!`/`crate::diagnostic!`, which writes standard error exactly as before and also the redacted local log.

Quick tier: [37887671760](https://github.com/pane-app/pane/actions/runs/37887671760) — green.

Refs #257
Closes nothing (a follow-up fix).

Signed-off-by: hoangvu12 <hggaming91@gmail.com>
