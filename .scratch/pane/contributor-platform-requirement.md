# Cross-platform contributor requirement

Accepted user clarification, 2026-09-28, during ticket review:

> Make sure it works on other os too cuz devs in other os gonna join as well

Windows, macOS and Linux developers must be able to contribute from the early shared milestones. Existing Windows-only prototype evidence describes what has been tested; it does not permit deferring the other contributor environments until the end of development.

- Early tickets 03 and 04 in revision 3 require actual native builds and runs of Pane plus JS/TS/Rust examples on macOS and Linux, immediately after the first integrated Windows path.
- Common documented build/run/test commands, explicit per-OS prerequisites, portable entry points and native automated checks are part of the initial interaction and native contributor slices (01, 03, 04). Shared feature development depends on the native baselines; there is no separate tooling-only milestone.
- Shared SDK/runtime/UI/build changes preserve all three baselines. Watchers, process invocation, paths and native helper artifacts account for each supported platform. Author examples and instructions accompany each language, reload, view, helper and distribution feature; assembled author-path verification is in the release checklist.
- Native GUI smoke evidence is separate from headless compile/contract checks. A missing target machine/runner or failed run leaves the gate open. Cross-compilation alone cannot prove native behavior.
- Exact OS versions, architectures and Linux desktop/display combinations must be stated and validated. This does not claim universal Linux desktop support or authorize infrastructure provisioning.
- Release timing can remain staggered. All previews depend on early cross-platform contributor support and the shared feature contracts. Each platform has its own installer, updater and release evidence; none waits for a different platform's completed installer, updater, benchmark or release approval. All platforms share the project; no long-lived platform fork is required.

This is an acceptance addendum to the [existing specification](spec.md), whose published file remains unchanged. The [ticket index](ticket-breakdown.md) links the 52 published implementation issues, each marked `ready-for-agent` and subject to its blockers. Only Windows prototype evidence currently exists; this revision adds requirements and performs no runtime tests.
