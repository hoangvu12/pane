# Pane: extensible desktop launcher

Status: ready-for-agent
Type: specification
Created: 2026-09-28
Scope: Product specification and bounded architecture-validation work for the initial launcher.
Testing approach: Proposed during spec synthesis; user feedback is tracked in Comments. It is not a claim of prior approval or completed tests.

## Problem Statement

Users want a fast, general-purpose desktop launcher that works across Windows, macOS and Linux without accumulating a large permanent feature core. They need everyday actions such as launching applications, calculations, quicklinks and file search, while retaining control over which features run.

Extension authors need room to build substantial functionality, reuse other extensions, use compatible libraries and native integrations, and reload their code without restarting the launcher. Users should be able to install supported extensions without manually installing programming runtimes or compilers.

Pane combines a Raycast-like interaction model with Pi-inspired extensibility and a small core. The existing work establishes product decisions and bounded Windows feasibility results. It does not yet provide a production launcher, completed SDK, integrated runtime/UI bridge or validated macOS/Linux support.

## Solution

Build Pane as a native GPUI CE launcher with a small core that coordinates search, navigation, commands, extension management and lifecycle behavior. Supply everyday features through independently disableable default extensions. Keep AI optional through extensions.

Support JavaScript, TypeScript and Rust extension authors at launch. The extension entry point must use WASI 0.3 with component-native async. WIT bindings and Wasmtime are the current architecture under evaluation; the JS engine and production runtime arrangement remain validation work. Allow optional prebuilt native helpers for functionality unavailable inside the guest.

Extensions are trusted local code. Prioritize their capabilities while managing activation, resource ownership and cleanup for reliability. Installation supports npm, Git and local sources without a compulsory catalog. Normal users receive supported runnable packages and automatically managed execution dependencies.

Prefer an internet-first installer that acquires runtime and default-feature payloads separately. Provide compatible automatic extension updates with controls, while allowing users to choose when to install updates to Pane itself. Recover through the UI when an identifiable extension fails, preserving saved data and keeping the core usable.

Windows is the first evidenced platform. Windows, macOS and Linux remain product targets; previews may ship separately as each platform is validated. This spec preserves unsettled implementation details as explicit gates instead of silently choosing them.

## User Stories

The stories express the intended product contract, not a claim that the prototype already implements it.

### Everyday use and the small core

1. As a user, I want a general-purpose launcher named Pane, so that I have one place to find and invoke everyday actions.
2. As a user, I want Pane to target Windows, macOS and Linux, so that I can use the same product across my machines.
3. As a user, I want consistent behavior where platforms permit it and clear explanations of differences, so that unsupported actions do not look successful.
4. As a user, I want a small permanent feature core and low resource usage, so that optional functionality does not continually increase the baseline cost.
5. As a user, I want to launch applications from root search, so that I can open them quickly.
6. As a user, I want calculator results in root search, so that I can perform quick calculations.
7. As a user, I want quicklinks in root search, so that I can invoke frequent destinations or actions.
8. As a user, I want enabled file results in root search, so that I can find local files.
9. As a user, I want installed extension commands to be discoverable, so that I can use an extension without remembering its implementation details.
10. As a user, I want aliases, hotkeys and fallback actions to help me reach commands, so that common workflows take fewer steps.
11. As a user, I want online service content searched inside its selected command, so that every root query does not automatically query all integrations.
12. As a user, I want to disable any default extension individually, so that I control which everyday features are active.
13. As a user, I want AI functionality to be optional extensions, so that Pane remains useful without AI services.
14. As a user, I want installed-but-unused extensions to remain inactive unless their declared work requires activation, so that installation alone does not imply continuous execution.

### Installation, identity and dependencies

15. As a user, I want internet-first setup to acquire Pane's execution dependencies for me, so that a smaller installer does not create manual setup work.
16. As a user, I want supported packages to work without manually installing Node, Rust, npm, Git or a compiler, so that using Pane does not require a developer environment.
17. As a user, I want to install extensions from npm, Git or a local source, so that installation is not dependent on a central catalog.
18. As a user, I want unavailable or incompatible artifacts explained, so that a source-only or unsupported package does not silently become a broken normal installation.
19. As an extension author, I want to use my own build tools when developing source packages, so that the end-user setup promise does not prohibit source development.
20. As a user, I want package identity derived from its source independently of its displayed title, so that changing a title does not redefine the tracked installation.
21. As a user, I want a second explicit install of the same canonical source rejected clearly, so that duplicate requests do not produce ambiguous installations.
22. As a user, I want ordinary updates to retain the tracked package identity, so that a version change is handled as an update.
23. As a developer, I want npm, Git and local copies to remain distinct when their source identities differ, so that I can control development and published installations explicitly.
24. As a developer, I want to enable, disable or remove those copies manually, so that development does not automatically swap my published installation or merge its data.
25. As an extension author, I want to declare required and optional extension dependencies, so that integrations can express which other extensions they need.
26. As a user, I want compatible missing required extensions shown and installed with my requested package, so that I do not have to assemble required dependencies manually.
27. As a user, I want optional integrations to remain optional, so that installation does not add unrelated extensions automatically.
28. As a user, I want deliberate disablement and version pins respected during dependency resolution, so that installation does not override my choices.
29. As a user, I want dependency version/platform conflicts explained, so that I can resolve an unavailable integration.
30. As a user, I want affected required dependents shown before disabling or uninstalling a dependency, so that I can choose the whole operation or cancel.
31. As a user, I want re-enabling or reinstalling a dependency to leave previously disabled or removed dependents under my control, so that recovery does not silently restore them.

### Extension authoring and interaction

32. As an extension author, I want JavaScript, TypeScript and Rust authoring support at launch, so that I can use a supported language without waiting for a later SDK.
33. As an extension author, I want an explicit Pane API, so that I target Pane's contracts rather than assume another launcher's API compatibility.
34. As an extension author, I want a WASI 0.3 component entry point, so that the host and guest use the selected runtime interface.
35. As an extension author, I want native component async behavior, so that asynchronous work fits the selected WASI3 model.
36. As an extension author, I want convenient standard UI controls rendered through GPUI CE, so that common views require little custom UI work.
37. As an extension author, I want custom interactive views, so that functionality is not restricted to fixed list templates.
38. As an extension author, I want supported capabilities available from JS/TS and Rust through documented contracts, so that language choice does not create accidental product differences.
39. As an extension author, I want to use compatible libraries, so that I can reuse existing functionality while understanding runtime limitations.
40. As an extension author, I want trusted access to files, networking, subprocesses and native integrations where supported, so that a mandatory capability-grant system does not define the initial product.
41. As an extension author, I want optional prebuilt native helpers invoked through host APIs, so that OS functionality and native libraries unavailable in the guest remain usable.
42. As a user, I want those helpers supplied for supported OS/architecture combinations without compiler setup, so that native integrations retain the normal installation experience.
43. As an extension author, I want to declare supported operating systems and action availability simply, so that unsupported actions can explain their limitations.
44. As a user, I want the supported portion of an extension to remain useful when another action is unavailable, so that one platform-specific feature does not hide working functionality.
45. As an extension author, I want lazy commands/events, scheduled work and explicit continuing background services, so that each workflow has an appropriate activation model.
46. As an extension author, I want to publish selected programmatic operations with structured inputs and results, so that other extensions can reuse my functionality.
47. As an extension author, I want to call those operations across JS/TS and Rust and receive completion or errors, so that composition is not tied to one language.
48. As an extension author, I want missing, disabled or incompatible targets reported without silently enabling them, so that calls respect the user's installation state.
49. As an extension author, I want UI commands to become programmatic APIs only when explicitly exposed, so that an interactive command is not assumed to support headless invocation.
50. As an extension maintainer, I want Pane to minimize unnecessary API changes while documenting actual incompatibilities, so that I can maintain my extension without an indefinite compatibility promise.

### Development, lifecycle and data

51. As an extension developer, I want saving code to build and reload only the affected extension, so that I can iterate while Pane stays open.
52. As an extension developer, I want a manual reload action, so that I can request replacement explicitly.
53. As an extension developer, I want failed builds to preserve the currently working instance and show diagnostics, so that a compilation error does not remove working functionality.
54. As an extension developer, I want a successfully built replacement to clean up the old instance before starting, so that reload has a defined lifecycle.
55. As an extension developer, I want saved data preserved through reload and transient-state restoration to be explicit, so that state behavior is predictable.
56. As an extension developer, I want replacement startup failures to show Retry and logs without automatic version rollback, so that recovery follows the agreed policy.
57. As a user, I want disabling an extension to stop its host-managed activation, running work, timers, subscriptions, commands and hotkeys, so that disabled means inactive through the host.
58. As a user, I want disabled state to survive restarting Pane, so that a restart does not undo my choice.
59. As a user, I want disablement to retain settings and unexpired data, so that I can re-enable the extension without starting over.
60. As a user, I want cache clearing to preserve settings, content and credentials, so that maintenance does not erase durable information.
61. As a user, I want uninstall to remove managed code, caches and locally managed credentials while offering a durable-data choice, so that deletion has a clear scope.
62. As a user, I want retained managed data removable even after an extension is gone, so that uninstall does not leave data I cannot manage.
63. As a user, I want my external documents and developer source folders preserved during uninstall, so that removal stays within Pane's managed scope.
64. As a user, I want cleanup to work without executing the broken or removed extension, so that recovery does not depend on that extension cooperating through its normal entry point.

### Clipboard history

65. As a user, I want clipboard history off until I enable it, so that capture is opt-in.
66. As a user, I want capture pause and disable controls, so that I can stop observation when needed.
67. As a user, I want configurable finite retention and per-item or bulk deletion, so that history does not grow indefinitely.
68. As a user, I want retention expiry to continue while the extension is disabled or Pane is stopped, so that re-enabling does not revive expired history.
69. As a user, I want Clear history and Disable and delete history to have distinct behavior, so that I can choose whether future capture continues.
70. As a user, I want available sensitive-content markers and app exclusions respected, so that supported platform signals can reduce unwanted capture without claiming perfect secret detection.
71. As a user, I want Pane's clipboard-history feature to avoid automatically sending history to AI or network services, so that local capture is not itself an online integration.

### Updates, failures and release clarity

72. As a user, I want compatible unpinned published extensions updated automatically, so that routine extension maintenance is convenient.
73. As a user, I want global/per-extension update controls and manual updates, so that I can choose how extension changes arrive.
74. As a user, I want pins and local development folders excluded from automatic replacement, so that updates respect explicit versions and source work.
75. As a user, I want updates to avoid replacing an extension in the middle of an active command, so that current work is not interrupted by routine maintenance.
76. As a user, I want Pane to notify me when an application update exists and let me choose installation, so that the app does not update or restart itself without my action.
77. As a user, I want an identified broken extension skipped automatically with a UI explanation, so that I can continue using Pane without finding a recovery CLI command.
78. As a user, I want Retry and diagnostic information while saved data remains intact, so that I can recover from an extension failure.
79. As a user, I want repeatedly failing background activation suppressed, so that one broken extension does not create an endless failure loop.
80. As a user, I want failures described according to what Pane actually knows, so that an unattributed shared-runtime crash does not falsely blame a particular extension.
81. As a user, I want platform support and preview limitations stated accurately, so that a Windows test result is not presented as verified macOS/Linux support.
82. As a contributor, I want the project's license direction and any component distinctions documented, so that reuse and contribution follow the chosen terms.

## Implementation Decisions

This section separates accepted product constraints from provisional implementation choices. The conceptual responsibilities below do not prescribe a file layout, final wire schema, class hierarchy or process count.

### Accepted product and architecture constraints

1. **Small core and extension-owned features.** The core coordinates the launcher shell, root search, command registry, navigation, action dispatch and extension management. App launching, calculator, quicklinks, file search and optional clipboard history are default extension functionality and individually disableable. AI stays in extensions. Default provision does not mean embedding every feature in the installer or permanently activating it.

2. **GPUI CE rendering.** GPUI CE is required. Provide standard controls plus custom interactive extension views for both language families. The concrete control set, view/event/drawing contract and input/accessibility behavior must be designed and validated; selection of GPUI CE does not establish full extension access to arbitrary renderer objects.

3. **Root-search behavior.** Aggregate applications, command metadata, quicklinks, calculator answers and enabled file results. Online service contents are queried inside the selected integration command. Aliases, hotkeys and fallback actions are part of the access direction; exact binding/ranking behavior remains design work. Index contribution metadata without assuming every extension is executing.

4. **Trusted capability model.** Extensions are trusted local code. Filesystem, networking, subprocess and native functionality are permitted by the product model, subject to OS/runtime support. Mandatory per-extension security grants and hardening are not prerequisites to initial capability. Managed lifecycle ownership serves reliability; it does not contain arbitrary detached programs or undo external side effects.

5. **Languages and WASI3.** Launch authoring languages are JS, TS and Rust. Require WASI 0.3 interfaces and component-native async at the extension entry point. Mixed P2/P3 output does not pass. WIT/Wasmtime is the current runtime direction under evaluation; earlier managed Node workers and native Rust executable entry points are historical baselines. Python/C# are later.

6. **Optional native helpers.** Allow prebuilt OS/architecture-specific helper programs invoked through host APIs. Manage their launch, input/output, cancellation and cleanup on disable/reload. This supports native functionality behind the component entry point; it does not restore a separate default native extension format. Do not promise universal cleanup of detached descendants.

7. **Own SDK and external sources.** Use Pane's API with npm, Git and local distribution. A central reviewed store is not required. Source builds and trusted build hooks are not categorically prohibited, but an arbitrary source-only package is not automatically a supported toolchain-free end-user release. No unchanged Raycast/Pi API or universal Node/npm compatibility is promised.

8. **Source-derived package identity.** npm identity is package name without version; Git identity is normalized repository host/path without ref; local identity is resolved absolute source location. Equivalent Git transport forms identify the same repository. Titles are separate. Different source kinds can be distinct copies. Reject duplicate explicit installs of the same canonical identity; normal tracked updates remain supported. Universal publisher/name IDs, generated permanent UUIDs and automatic source/data merging are not adopted.

9. **Developer-controlled copies.** Local development and published copies are independently controlled. Starting/stopping development does not automatically swap, disable/restore or remap the published copy.

10. **Dependency policy.** Show and install compatible missing required extensions with the requested package. Optional integrations remain optional. Respect deliberate disablement and pins; explain incompatible or unavailable requirements. Distinguish launcher-extension dependencies from ordinary source-library dependencies. Exact addressing, version resolution, cycles and partial-install behavior remain open.

11. **Dependent disable/removal.** Before normal user-requested disable/uninstall of a dependency, show the required dependent set and offer Disable All/Cancel or Uninstall All/Cancel. Apply the existing data policy to each affected extension. Re-enabling/reinstalling the dependency alone does not restore dependents. Optional integrations do not cause required-dependent removal.

12. **Explicit cross-extension operations.** Route structured calls and awaited results/completion/errors through the host between JS/TS and Rust. Authors explicitly expose operations; an ordinary UI command is not automatically a programmatic API. Missing, disabled or incompatible targets produce clear failures rather than silent re-enablement. No visual workflow editor is implied.

13. **Lazy and background activation.** Support lazy commands/events, schedules and explicit ongoing services. Installation alone does not keep executable instances alive. Disabling stops host-managed work. Idle lifetime and scheduling intervals are not yet selected.

14. **Reload contract.** Development saves build and replace only the affected extension; manual reload is available. The launcher stays open. Build failure preserves the old working instance and reports diagnostics. Successful build proceeds through old-instance cleanup to replacement startup. Saved data persists; transient restoration requires explicit extension support. Replacement startup failure cleans up managed work and exposes Retry/logs without automatic version rollback or data-migration reversal.

15. **Disable and data ownership.** Disable stops host-managed activation, in-flight work and contributions, retains settings/unexpired data and persists across restart. Re-enable uses retained data after expiry handling. Clear cache preserves durable settings/content/credentials. Uninstall removes managed code/cache/local credentials and offers a durable-data choice. Provide later deletion of retained managed data without the extension present. Preserve user-owned development sources and unrelated external content. Local credential deletion is distinct from revoking remote sessions.

16. **Clipboard behavior.** Off until enabled, with pause/disable, finite configurable retention, item/bulk deletion and Disable and delete history. Clear history does not disable later capture. Expiry remains effective while disabled and is enforced before display after downtime. Respect available sensitive markers/exclusions without guaranteeing detection of every secret. Do not automatically send history to AI/network integrations; full-trust code is not thereby isolated from it. The default retention duration remains open.

17. **Two update policies.** Automatically update compatible unpinned published extensions, with global/per-extension opt-out and manual updates. Preserve pins/local development folders and avoid replacement during active commands. Pane application updates instead notify users and await their installation choice; automatic application downloads or restarts are not accepted. Runtime update delivery and activation timing for long-lived views/services remain open.

18. **Automatic UI recovery.** Detect and skip an identified broken extension, notify through the UI, retain data and keep Pane usable. Retry/logs and suppression of repeated background failures remain required direction. User-requested dependency removal is distinct from a runtime failure. Detailed failure state, attribution, persistence and retry thresholds remain proposed; a shared runtime crash may have no identifiable single culprit. No ordinary-user CLI requirement.

19. **Simple platform availability.** Support OS declarations and straightforward per-action availability explanations, preserving working actions. Do not introduce a general compatibility-rule language. Helper artifacts match supported platform/architecture combinations. Core product targets remain Windows/macOS/Linux; only Windows prototypes have been validated, and previews may be staggered.

20. **Internet-first normal setup.** Prefer leaving runtime/default-feature payloads out of the installer and acquiring them automatically. Normal supported-package users manually install no runtimes, package managers or compilers. Initial connectivity is assumed; complete offline first use is not promised. Source authors may need tools, and integrations may require external accounts/apps. Runtime management should preserve the user's existing developer environment.

21. **API maintenance.** Minimize unnecessary changes while allowing breaking evolution. Extension authors maintain their packages; Pane does not take over abandoned extensions or retain obsolete interfaces indefinitely. Compatibility declaration/version signaling must support meaningful install/update diagnostics; exact versioning and deprecation mechanics remain open.

22. **Name and provisional license direction.** The product name is Pane. Follow Zed's primarily GPL-3.0-or-later direction provisionally, with exact component/SDK licensing and dependency compatibility to resolve. This does not prohibit compliant paid forks or automatically assign one license to every third-party extension. No license has yet been applied.

### Proposed responsibility boundaries

These are a design sketch for decomposition, not new accepted wire contracts:

| Responsibility | Owns | Key external behavior |
| --- | --- | --- |
| Launcher core and renderer | Search/navigation, command metadata, native views, settings and extension-management UI. | User actions remain available independently of a broken extension. |
| Extension lifecycle and runtime | Activation, guest generations, managed work, calls, replacement and failure reporting. | JS/TS and Rust follow the same lifecycle policy through the supported runtime interface. |
| Package management | Source identity, artifacts, compatibility, dependencies and update staging. | Install/update/disable/removal obey declared support, pins and user choices. |
| Managed persistence | Settings, durable content, disposable cache, local credentials and retention. | Ownership and deletion remain usable when an extension is disabled or absent. |
| Platform integration | Native UI/OS integration, helper processes and capability availability. | Supported behavior is explicit and tested per platform. |
| Author tooling and SDK | Bindings, package preparation, save/build/reload and diagnostics. | Authors can produce supported packages and exercise the same host contracts. |

Keep these responsibilities behind cohesive public interfaces rather than exposing engine-specific objects. Their final arrangement may be smaller or combined. Do not infer a new process or independently deployable service from each row.

### Provisional runtime/recovery mechanics

A managed Wasmtime helper, a shared engine, and per-extension-generation stores are candidates. They are not a measured production topology. QuickJS is one tested JS backend, not a requirement or an approved permanent fork.

The proposed recovery mechanics distinguish expected operation errors from fatal initialization or repeated execution failures, preserve a failure status after a toast disappears, and provide Retry/Details. A persistent activation record may help avoid known crash loops, but an interrupted startup or last-active extension is not proof of causality. If attribution is unavailable, report a runtime failure rather than inventing a culprit. Restarting execution infrastructure should not automatically repeat user actions with potentially completed external effects. Validate these mechanics before depending on them.

The WIT probe's async string query and the renderer probe's JSON input were feasibility interfaces. They are not selected as the public SDK or production IPC format. Public serialization, identifiers, cancellation and UI schemas still need design.

### Decisions and validation gates before dependent implementation

| Gate | Work needed | Completion evidence |
| --- | --- | --- |
| G1: Production runtime candidate | Select viable JS backend/toolchain strategy while preserving JS/TS, Rust and pure WASI3; resolve snapshot initialization and maintenance ownership. | Representative guests run under a P3-only host, the mixed-version control is rejected, fresh instances have correct initialization, and costs/compatibility limitations are recorded. |
| G2: SDK and native UI contract | Define shared host/guest operations and views/events, cancellation, custom interaction and native-helper access. | JS/TS and Rust drive the same supported contracts, including a real view/event round trip; focus/text input/IME/accessibility and representative custom interaction are validated. |
| G3: Lifecycle and recovery | Define generation ownership, cancellation/hangs, helper cleanup, repeated failure behavior and attributable versus shared-runtime failure handling. | Actual guest replacement/disable and representative crash cases obey policy without stale replies, repeated activation loops or automatic replay of side effects. |
| G4: Packages and compatibility | Define manifests, artifacts, resource/operation addressing, version declarations, dependency cycles/partial installs, Git tracking and update staging. | Supported local/npm/Git fixture packages exercise the agreed identity/dependency/update outcomes and incompatibility diagnostics. |
| G5: Data and retention | Define managed storage/credentials, migration ownership, expiry and concrete clipboard defaults. | Data survives allowed lifecycle changes; scoped deletion/expiry works with disabled or missing code and after restart. |
| G6: Installer and update delivery | Define runtime/default acquisition, cache/retry behavior and app/runtime update mechanics. | A clean supported machine completes normal setup without manual developer tools; user-controlled app installation and separate extension-update behavior are demonstrated. |
| G7: Platform and resource claims | Establish support matrix, measurements and explicit performance targets. | Each claimed OS/architecture/desktop combination has native results; measure the full process tree, cold/warm start, inactive extensions and repeated lifecycle work. No target is inferred from a toy benchmark. |
| G8: Licensing | Select exact first-party application/SDK/component terms and review dependencies. | Component notices and distribution obligations are consistent with the provisional Zed-style direction; extension licensing assumptions are explicit. |

The spec can be decomposed into bounded decision/validation work and implementation work. A gate is not permission to silently relax an accepted product requirement. If a proposed contract changes a consequential product choice, return that decision to the user before dependent implementation. Gate completion may overlap where prerequisites permit it; this table does not impose a serial waterfall.

## Testing Decisions

### Proposed primary test boundary

Use the highest practical public host interface: drive launcher and extension-management actions, then assert observable results, view state/events, installation state, managed data and process/resource effects. Execute real representative WASI3 guests rather than replacing the runtime with mocks in the main integration path.

This interface must be designed as part of the production core; it does not exist as a reusable production test harness today. Prefer a single integration entry point for search/run, composition, reload, disable, installation and recovery wherever practical. It should exercise the same behavior used by the UI without requiring tests to inspect private renderer objects, engine internals or worker layouts.

Use deterministic boundaries for external package sources, clocks and OS/service effects where necessary. Add lower-level tests only where a public contract or difficult algorithm warrants them; avoid tests that merely mirror internal methods. Tests should remain meaningful if the engine or process arrangement changes while behavior stays the same.

### Necessary native checks

The primary host interface does not establish rendering or installation correctness. Add focused real GPUI UI checks for display, keyboard/pointer interaction, focus/text input/IME and failure notifications, plus clean-machine setup and platform integration checks on each claimed target. These supplement the main boundary; they do not require every behavior test to drive coordinates in a native window.

Use representative simple and custom views from both language families. Exact initial controls and accessibility expectations must be resolved under G2 before claiming UI parity.

### Observable acceptance scenarios

| ID | Scenario | Required observation |
| --- | --- | --- |
| T01 | Start Pane with default features individually enabled/disabled. | Root search reflects enabled contributions; disabled features do not perform host-managed work. |
| T02 | Install many extensions but invoke only selected commands. | Installation alone does not activate all executable instances; enabled declared background work remains distinguishable. |
| T03 | Search root and an online integration command. | Local/default results follow policy; online service-content queries are scoped to the selected command. |
| T04 | Run representative JS/TS and Rust components under P3-only registration. | Supported operations succeed; stock mixed P2/P3 imports are rejected rather than silently accommodated. |
| T05 | Render and interact with both language families' views. | Real guest events return through the production boundary and update the view; file replacement alone does not satisfy this scenario. |
| T06 | Use async calls, file streams, repeated calls and fresh instances. | Results, errors, time, initialization and managed resources follow the documented contract; reproduce and resolve the saved snapshot-randomness defect. |
| T07 | Save valid and invalid development changes. | Invalid builds keep the old guest; valid builds replace the affected guest without restarting Pane or unrelated extensions. |
| T08 | A new guest builds but fails during startup. | Failure is shown with Retry/logs and managed cleanup, saved data remains, and no automatic version rollback occurs. |
| T09 | Disable/reload while async work is pending. | Old work is stopped through the host and stale results do not mutate the replacement/current view. Include native helper and stream cleanup. |
| T10 | Restart after a deliberate disable. | The extension stays disabled; settings/unexpired content remain; expired records are not revived. |
| T11 | Install equivalent and distinct source references. | Equivalent canonical sources reject duplicate installs; version updates remain updates; distinct npm/Git/local copies can coexist without automatic merging. |
| T12 | Install required and optional dependencies, including disabled/pinned conflicts. | Compatible missing required dependencies are shown/installed; optional ones are not automatic; disabled choices/pins are preserved and conflicts explained. |
| T13 | Disable/uninstall a required dependency and choose each offered outcome. | Cancel changes nothing; confirmed operations affect the shown required set; later restoring the dependency does not restore dependents automatically. |
| T14 | Call explicit operations across JS/TS and Rust. | Structured results/errors return; missing/disabled/incompatible operations report correctly; UI-only commands are not implicitly callable APIs. |
| T15 | Automatically update extensions under enabled/disabled controls, pins and local development. | Only eligible packages are replaced, and active commands are not interrupted by the replacement. |
| T16 | Discover a Pane application update. | The UI notifies the user; download/install/restart behavior does not silently adopt automatic app updates. |
| T17 | Trigger an identified startup failure, repeated background failures and ordinary operation errors. | Automatic UI recovery skips the identified broken extension; ordinary errors do not indiscriminately disable healthy functionality. Validate proposed pause/retry persistence under G3. |
| T18 | Terminate or hang a shared runtime with multiple extensions active. | Core recovery remains available; diagnostics reflect actual attribution limits. Validate the chosen supervision boundary without claiming arbitrary-code containment. |
| T19 | Lose a response after a representative side-effecting operation. | Recovery does not blindly replay the user's action. |
| T20 | Clear cache, uninstall with each data choice, and delete retained data while code is absent. | Each operation respects managed ownership and preserves unrelated documents/development sources. |
| T21 | Enable, pause, disable, clear and expire clipboard history. | Capture and retention match policy, including disabled/downtime expiry and the difference between Clear and Disable and delete. |
| T22 | Run platform-limited actions and helpers. | Availability explanations are accurate, functioning actions remain usable, and supported artifacts require no end-user compilation. |
| T23 | Complete installation on a clean supported machine with an interrupted initial download. | No manual developer/runtime tools are needed; the selected setup/retry design gives a recoverable outcome. The precise UI follows G6. |
| T24 | Repeat activation, calls, reload and disable under representative load. | Collect sustained memory/CPU/latency and resource-release evidence across the process tree; compare with targets established under G7. |
| T25 | Validate each advertised release target. | Native UI/OS/install/lifecycle results exist for that target; Windows evidence alone is insufficient for macOS/Linux claims. |

These are planned checks, not test results. The exact retry thresholds, timeouts, resource budgets and several schemas remain gate outputs.

### Existing prior art and its limits

- The P3-only embedded host already accepts real Rust and patched JS components and rejects the stock mixed guest. Reuse the positive/negative contract pattern, not its debug executable size or ad hoc argument interface as a product design.
- The Rust std experiment exercises filesystem read/write/delete, environment, clocks and diagnostics. It does not prove every Rust crate or async ecosystem works.
- The patched QuickJS experiment exercises Fuse/Zod, async streams, 20 sequential calls in one instance, missing-file/no-preopen cases and a fresh-instance initialization defect. It does not prove concurrency, cancellation or leak-free lifecycle behavior.
- The GPUI experiment renders actual guest-produced data and records native click events on Windows. Guest execution and UI interaction were separate file-connected steps; it is not a production round trip or guest-code hot reload.
- Saved benchmarks describe small-sample CLI runs with differing optimization settings. They are evidence for further measurement, not launcher idle-memory targets or additive per-extension RAM.

### Testing-boundary check

The user was asked whether the proposed public-host integration boundary plus focused native UI/installer checks matches their expectations. This request concerns test organization, not reopening the accepted product choices. Until answered, the approach remains the explicit recommendation above; writing this spec does not assert that it was approved.

## Out of Scope

- A reviewed/browsable extension catalog or store for the initial functionality milestone.
- Automatic historical-version discovery or a dedicated downgrade picker. Existing pins and explicit supported version/ref inputs remain in scope.
- Python/C# launch SDKs; JS/TS and Rust remain required.
- Unchanged Raycast/Pi extension compatibility or universal Node/npm/native-library compatibility.
- Restoring managed Node workers/native Rust executable entry points as the default architecture, or silently accepting mixed WASI2/WASI3 output.
- Mandatory per-extension security grants, a hardening-first project or guarantees against arbitrary trusted code's external effects.
- Automatic application update installation/restarts, automatic version rollback after replacement startup failure, or reversal of data migrations/external side effects.
- Automatic switching/merging between development and published copies, or restoring dependents merely because their dependency returns.
- Guaranteed preservation of arbitrary live guest state across reload.
- Automatic root-search queries across every online integration.
- A general compatibility-rule language, general multiple-version dependency solver or visual workflow editor.
- A separate mandatory AI subsystem.
- A simultaneous three-platform release, offline first-use guarantee, arbitrary source-only end-user builds without tools, or unmeasured performance guarantees.
- A ban on compliant commercial forks under the provisional open-source licensing direction.
- Starting application implementation, creating tickets, rerunning experiments or publishing externally as part of this spec-writing task.

## Further Notes

### Readiness and current state

This is a local tracker specification synthesized from the completed main grilling round and the pre-spec audit. The ready-for-agent label means it can be consumed for decomposition and bounded next work. It does not mean all gated engineering decisions are already settled, every proposed detail is user-approved, or the product is ready to ship.

The current workspace contains research documentation and throwaway prototypes. There is no production application or completed extension SDK to extend, and no existing Git repository/remote. Some build tools and executable artifacts are under temporary storage. Preserve the source evidence and pinned reproduction information without treating temporary artifacts as a durable distribution pipeline.

QuickJS is provisional. Its temporary integration patch removes the old adapter path, links P3 libc, removes P2 host registration, exports TLS metadata and separates async task bookkeeping from libc's canonical context storage. Fresh instances currently repeat snapshotted Math.random state. No long-term fork is selected.

### Decision traceability

| Source decisions | Spec coverage |
| --- | --- |
| Q1-Q3, Q5-Q6, Q8 | Product, small core, default features and platform consistency. |
| Q4, Q7 | Historical comparison requests; later explicit choices govern. |
| Q9-Q11 | Trust/distribution and disable/data policy. |
| Q12-Q13 | Launch languages and GPUI CE extension UI. |
| Q14-Q17 | Reload, activation, managed setup and root search. |
| Q18-Q19, Q22-Q23 | Historical runtime choices, explicitly superseded by the later WASI3 direction. |
| Q20-Q21 | Failure handling and controlled automatic extension updates. |
| Q24-Q27 | API evolution, composition, dependency installation and dependent removal. |
| Q28-Q30 | Source identity, duplicates and manual development-copy control. |
| Q31-Q32 | Failed replacement behavior and deferred history picker. |
| Q33-Q35 | Native helpers, simple platform availability and deferred catalog. |
| Q36-Q38 | Internet-first setup, staggered previews and user-initiated app updates. |
| Q39-Q41 | Automatic UI recovery, provisional Zed-style licensing and Pane name. |
| Later runtime corrections | Pure WASI3 requirement, provisional QuickJS, limits of existing tests and deferred experimentation. |
| Workflow choices | Local Markdown tracking, default triage roles and both agent-instruction entrypoints. |

### Source record

The following are provenance links, not prescribed implementation file locations:

- [Current decision index](../../docs/current-decisions.md).
- [Detailed extension policies](../../docs/extension-policy-proposal.md).
- [Saved interview and later corrections](../../docs/launcher-design-interview.md).
- [Saved-record limitations](../../docs/current-decisions.md#record-limits).
- [ADRs](../../docs/adr/), especially [small core](../../docs/adr/0001-small-core.md), [trust](../../docs/adr/0002-trusted-extensions-and-open-distribution.md), [GPUI](../../docs/adr/0003-gpui-ce-and-extensible-views.md), [reload](../../docs/adr/0004-reload-extensions-without-restarting-launcher.md), [activation/setup](../../docs/adr/0005-lazy-activation-and-managed-dependencies.md), [search](../../docs/adr/0006-raycast-style-search-with-extension-providers.md), [API evolution](../../docs/adr/0010-best-effort-extension-api-compatibility.md), [composition](../../docs/adr/0011-extension-call-and-result-api.md), [identity](../../docs/adr/0012-pi-style-source-identity.md), [WASI3](../../docs/adr/0013-require-wasi-03.md) and [helpers](../../docs/adr/0014-optional-native-extension-helpers.md).
- [Aggregate validation checkpoint](../../docs/research/wasi03-validation.md).
- [P3-only embedded host](../../docs/research/p3-only-host/README.md).
- [Rust std probe](../../docs/research/p3-std-spike/README.md).
- [QuickJS P3 port, patches and limitations](../../docs/research/qjs-p3-port-spike/README.md).
- [GPUI Windows probe](../../docs/research/wasi03-gpui-spike/README.md).
- [License comparison and provisional selection](../../docs/research/pane-license-options-q40.md).

## Comments

2026-09-28: Created through the user-invoked to-spec workflow after auditing continuity across compactions. A testing-boundary preference question was presented while this specification was drafted. No unanswered preference has been recorded as an accepted decision.
