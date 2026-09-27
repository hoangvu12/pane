# Cross-platform contributor requirement

Accepted user clarification, 2026-09-28, during ticket review:

> Make sure it works on other os too cuz devs in other os gonna join as well

Windows, macOS and Linux developers must be able to contribute from the early shared milestones. Existing Windows-only prototype evidence describes what has been tested; it does not permit deferring the other contributor environments until the end of development.

- Early tickets 05 and 06 require actual native builds and runs of Pane plus JS/TS/Rust examples on macOS and Linux, immediately after the first integrated Windows path.
- Ticket 07 requires a common documented build/run/test workflow, explicit per-OS prerequisites, portable task entry points and native automated checks suitable for the eventual CI provider. Shared extension development depends on this milestone.
- Shared SDK/runtime/UI/build changes preserve all three baselines. Watchers, process invocation, paths and native helper artifacts account for each supported platform. Later reload and author-onboarding tickets demonstrate the complete workflow on all three.
- Native GUI smoke evidence is separate from headless compile/contract checks. A missing target machine/runner or failed run leaves the gate open. Cross-compilation alone cannot prove native behavior.
- Exact OS versions, architectures and Linux desktop/display combinations must be stated and validated. This does not claim universal Linux desktop support or authorize infrastructure provisioning.
- Release timing can remain staggered. The Windows preview depends on early cross-platform contributor support, but not on later macOS/Linux feature parity, installer or release-readiness tickets. All platforms share the project; no long-lived platform fork is required.

This is an acceptance addendum to the [existing specification](spec.md), whose published file remains unchanged. The [revised breakdown](ticket-breakdown.md) is still a draft awaiting approval. Only Windows prototype evidence currently exists; this revision adds requirements and performs no runtime tests.
