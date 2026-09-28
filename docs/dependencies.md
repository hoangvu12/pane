# Dependencies on other extensions

Added for [#42](https://github.com/hoangvu12/pane/issues/42) (US25–US29, T12,
T14, G4), following the accepted Q26 direction in the
[extension policies](extension-policy-proposal.md#dependencies-on-other-extensions).
A package that calls other packages' [operations](operations.md) declares
them in its `pane.json`, required or optional. Installing it from a local
folder shows them first and installs the missing required ones with it;
optional, disabled and already installed dependencies are left as they are.
Disabling or uninstalling a required dependency together with its dependents
is [#43](https://github.com/hoangvu12/pane/issues/43) and
[#44](https://github.com/hoangvu12/pane/issues/44); npm sources are
[#45](https://github.com/hoangvu12/pane/issues/45).

## Declaring

```json
"dependencies": [
  {
    "id": "greeter",
    "source": "local:../sample-operations-js",
    "operations": [{ "id": "greet", "version": 1 }]
  },
  {
    "id": "rust-greeter",
    "source": "local:../sample-operations",
    "optional": true,
    "operations": [{ "id": "greet", "version": 1 }],
    "platforms": ["windows", "linux"]
  }
]
```

| Field | Meaning |
| --- | --- |
| `id` | The name the package's code calls the dependency by, unique in the package: lowercase letters, digits and `-`. It has no `:`, so it is never mistaken for a package identity. |
| `source` | Where it is installed from: `local:` and a folder path, relative to the declaring package's source folder or absolute. `npm:` and `git:` are accepted in the manifest but explained as not supported yet when Pane would have to install from them. Anything else is an invalid manifest. |
| `optional` | `false` (the default) for a required dependency, `true` for an optional one. |
| `operations` | Every operation the package calls there, each with the version it calls; at least one. This is the compatibility requirement: an operation's version is the version of its input and result ([operations](operations.md#publishing)). |
| `platforms` | Optional: the systems on which the package needs it. Elsewhere it is neither installed nor checked, and a call to it that finds it missing is `unavailable`. |

There is no version range on the package itself: a package's `version` is
free text, and what a dependent relies on is the operations it calls at
their versions. Pins of published versions come with npm sources (#45),
where a version exists apart from the identity.

## Addressing

Pane resolves a `local:` source against the declaring package's source
folder exactly as it resolves an installed folder (the operating system's
resolved absolute path, [identity](../guests/README.md#packaging-and-installing-a-local-extension));
a folder that does not exist is resolved from its spelling, `.` and `..`
removed. What each dependency resolved to is recorded with the package in
`installed.json` (`"dependencies": [{ "id": "greeter", "local": "/…" }]`),
so a source folder moved or deleted after installing changes nothing.

A guest calls a dependency with its id where it would give an identity:

```rust
call("greeter".into(), "greet".into(), 1, input).await
```

Pane finds the caller's package from the calling component, looks the id up
in its manifest and uses the recorded identity; the call then behaves as a
call by identity ([operations](operations.md#calling)). Calls by identity
still work, declared or not. The errors for an id:

| Kind | When |
| --- | --- |
| `not-found` | The caller declares no dependency with that id ("Caller declares no dependency `nobody` in its pane.json, …; it declares `greeter` and `helper`"), or it is not installed: for a required one "Caller requires `greeter` from local folder …, which is not installed; install Caller again to install it", for an optional one "Caller's optional dependency `helper` from local folder … is not installed; install it to use it". |
| `unavailable` | Its source is not a local folder (npm, Git), or it is needed only on other systems and not installed. |
| `disabled` and the rest | As for any call once the target is found. |

## Installing

Previewing a package (**Install extension from folder…** or
`pane --install <folder>`) works out its dependencies before offering
Install, reading only manifests and checking components without running
them, and lists them under the package's details:

- "Requires: JavaScript operations sample, installed with it from local
  folder …": missing, installed with it. The Install row then says "…, and
  install JavaScript operations sample, which it requires" (or "the N
  extensions it requires").
- "Requires: …, already installed": used as it is.
- "Requires: …, which you disabled: it stays disabled, and <title> cannot
  use it until you enable it in Manage extensions": the package is
  installed, the dependency is not enabled, and the status after installing
  says so again. Calls to it answer `disabled`.
- "Requires (for B): C, …": a dependency of a dependency Pane installs.
- "Optional: `rust-greeter` from local folder …, not installed: Pane does
  not install it; install it yourself to use it", or "Optional: <title>,
  installed" (or "installed but disabled").
- "Not needed on this system: `x` from local:../x (only on Windows)".

Installing (Enter on Install, or `Launcher::install_package`) works it out
again from the current state, then installs the missing required packages,
each after its own required dependencies, then the requested one, and shows
"Installed Caller with JavaScript operations sample, which it requires". An
Update does the same for a new copy that adds a required dependency
("Updated Caller with …").

### What stops an install

Every problem with a required dependency is found before anything changes;
the preview is then titled "Cannot install <title>" with no Install row, and
the status (also for an install without preview) starts "Nothing was
installed:", listing each:

- **Unavailable**: its folder is missing or not a package, it is
  source-only, it is for other systems, its components do not pass Pane's
  checks, or its source is npm or Git.
- **Incompatible**: it does not publish an operation the dependent calls, or
  publishes it at another version, or not on this system.
- **Pinned**: an installed dependency that is incompatible is not replaced:
  "…; Pane does not replace the installed copy of <title> while installing
  another extension: update it from its folder first". Installing never
  updates another package.
- **Conflicting**: two packages in the install need different versions of
  one operation from the same source: "A and C need different versions of
  `echo` from B (1 and 2); Pane installs one copy of each source, so they
  conflict". There is one installed copy per source and no solver choosing
  among versions.
- **Itself**: a package naming its own folder as a dependency.
- **Too many**: more than 16 packages would be installed with it
  (`MAX_INSTALLED_WITH`).

An optional dependency never stops an install, whatever its state.

### Cycles

Each source is visited once, so packages that require each other (A needs
B, B needs A) are installed together, A last; what each needs of the other
is still checked. An installed dependency's own dependencies are not
visited again: it was installed with them, or they were removed since
(#43, #44).

### Partial installation

Installing writes one package at a time. If one fails (a storage error, or
another install of the same source finishing first), Pane removes again the
packages this install added, most recent first, with their managed copies,
restoring the record of [retained data](extension-data.md#retained-data)
one of them had, so nothing is left installed without its required
dependencies and nothing claims to be ready. The status names what failed:
"Could not install C, which it requires: …". Only if removing one again
fails does it stay installed, and the status says so ("Pane could not
remove again what it had installed, which stays installed: B (…)"). None of
the packages ran meanwhile, so none has data to lose.

## Checks

- [`crates/pane-core/tests/dependencies.rs`](../crates/pane-core/tests/dependencies.rs)
  drives the preview, installing with a missing required dependency and the
  cross-language call by id (JavaScript calling Rust), optional, installed,
  disabled, pinned, conflicting and unavailable dependencies, cycles,
  dependencies of dependencies, an update adding one, a dependency
  uninstalled later, invalid declarations and the sample.
- Unit tests in [`dependencies.rs`](../crates/pane-core/src/dependencies.rs)
  inject a failing install to check the rollback, including retained data.
- The native smokes install the [dependencies sample](../guests/sample-dependencies/src/lib.rs)
  and show "Hello, Pane, from JavaScript" in the real window (frames 66 to
  68; [Linux](platforms/linux.md#dependencies-42)).

## Limits

- Local folders only; npm and Git (#45 and later) keep these semantics.
- One copy per source, no version ranges and no multi-version solving.
- No Pane-side view yet of installed packages whose required dependency was
  disabled or removed later; their calls explain it (#43, #44).
- The preview lists sources as full paths, which makes it long; the Install
  row can be pushed below the fold on a small window (it stays selected).
