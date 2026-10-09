# Extension API versions: a new version for every breaking change, and the previous one kept for a release
## Problem Statement

Extension API 0.1 has changed shape between slices without a new number (#19, #21, #29, #170). Pane catches a stale build by type-checking the component before running it ("built for an older extension API shape"), and so far every affected extension was rebuilt in the same change, because every extension lived in this repository. With official extensions in their own repositories (ADR 0045) and collections from anyone (ADR 0044), a silent shape change breaks extensions nobody can rebuild in the same change, and their users cannot tell why.

[ADR 0046](https://github.com/pane-app/pane/blob/main/docs/adr/0046-every-breaking-change-to-the-extension-api-gets-a-new-version.md) decides that every breaking change gets a new API version, and that Pane runs the previous version for a while. This specifies it.

## Solution

- The extension API version is the WIT package's version, and a manifest's `apiVersion` names the version an extension was built for. A breaking change bumps it: the minor before 1.0, the major from 1.0.
- CI refuses a change to the WIT that breaks the released API without a new version.
- Pane runs extensions built for its current version and its previous one, adapting the previous one in the host, and marks them on their pages.
- An extension built for a version Pane does not support is refused before any code runs, stays listed, and says what to do.
- Update checks never offer a version this Pane cannot run.
- Each release's notes list API breaking changes with code before and after.

## User Stories

1. As an extension author, I want every breaking change to come with a new API version, so that I know when I must change my extension.
2. As an extension author, I want my extension built for the previous API version to keep working for at least one Pane release after a new one, so that I have time to update it.
3. As an extension author, I want release notes listing each breaking change with code before and after, so that updating is mechanical.
4. As an extension author, I want my extension's page to tell users it was built for an older API, so that they know an update is coming rather than that it is broken.
5. As a launcher user, I want an extension Pane can no longer run to stay listed, disabled, with which version it needs and whether to update it or Pane, so that it never vanishes without a reason.
6. As a launcher user, I want update checks to skip a version that needs a newer Pane and say so, so that an update never breaks an extension.
7. As a maintainer, I want CI to fail when the WIT changes incompatibly without a new version, so that no shape change ships under an old number.
8. As a maintainer, I want the previous version's adapter to live in one place and be removed when its window ends, so that supporting two versions stays contained.

## Implementation Decisions

- **One number.** `wit/` declares `pane:extension@<version>`; `EXTENSION_API` in `packages.rs` is derived from it. `api_compatible` accepts the current version and the previous one (before 1.0: the previous minor; from 1.0: any minor of the current major, and the previous major during its window).
- **What breaks.** Removing or renaming an interface, function, type, field, case or flag; changing a parameter or result type; reordering a record or variant; changing what a call does in a way an existing extension would notice. Adding a function, a type or an interface is not breaking; adding a field to an existing record is, so a new field comes as a new type and function.
- **The CI guard.** A check compares `wit/` with the WIT of the last release tag (`wasm-tools component wit` or a structural diff of the parsed packages) and fails when the change is breaking and the version did not move. It runs with the lints.
- **The previous version.** The host keeps the previous WIT world under `wit/previous/` and links an adapter, written in Rust in the host, mapping its imports and exports onto the current ones. A component built for the previous version is instantiated against that world. The adapter and `wit/previous/` are deleted when the window ends.
- **The window.** Support for a previous version ends with the first Pane release after the one that replaced it (proposed by ADR 0046).
- **Explaining.** The refusal names the extension's version, Pane's versions and the remedy. The extension stays listed with the paused state's details. A previous-version extension shows "Built for extension API 0.1; Pane 0.2 runs it until its next release" on its page. The shape type check stays as the safety net.
- **Updates.** The updater (#127) and any catalog skip versions whose `apiVersion` this Pane cannot run, recorded as "needs a newer Pane".
- **Release notes.** A "Breaking changes" section per API version: each change with code before and after, for Rust and JS/TS.
- **The SDK.** The published SDK (#128) is versioned with the API version it targets, so an author upgrading the SDK knows which API they target.

## Testing Decisions

- `packages.rs` unit tests for `api_compatible` across current, previous, older and newer versions, before and after 1.0.
- Fixtures built for the previous version running through the adapter; fixtures built for an older version refused with the message.
- A test of the CI guard on a breaking and a non-breaking WIT change.
- Updater tests that a version needing a newer Pane is skipped with that reason.

## Out of Scope

- Migration codemods (#128 excludes them).
- Supporting more than one previous version.

## Further Notes

The first breaking change after this lands ships as 0.2. Pi, which has no API version and no load-time check, changed its extension API incompatibly in about 52 of its 288 releases; the research is summarized in ADR 0046.



## COMMENTS

