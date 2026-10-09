## Parent

https://github.com/pane-app/pane/issues/128

## What to build

The SDKs are publishable (#220: `cargo xtask sdks` checks both) but nothing is published, and publishing is a person's step, never CI's. Publishing them is what lets the official extension repositories (#207) build against the SDK without a Pane checkout, and what lets any outside author depend on it.

- `pane-extension` 0.1.0 published to crates.io — the package #220 verified builds from the package alone for `wasm32-wasip2`.
- `@pane-app/extension` 0.1.0 published to npm.

## Acceptance criteria

- [ ] `pane-extension` 0.1.0 is on crates.io and a fresh project builds a component with it
- [ ] `@pane-app/extension` 0.1.0 is on npm and a fresh project installs it
- [ ] #207's follow-up (build the official repositories against the published crate) is unblocked

## Blocked by

- None — a person's step, ready when they are.
