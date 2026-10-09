## Parent

https://github.com/pane-app/pane/issues/207

## What to build

Pane's repository no longer holds the five default extensions' sources: they live in their repositories (#279), Pane installs them from the commits this release pins (#278, #282), and the smokes serve those repositories (#284). The samples stay.

- The five extensions' source folders and package folders leave the guests tree: the guest build no longer builds them, the packaging no longer assembles them, the guest workspace no longer carries them. The samples, fixtures and helpers are untouched.
- The tests that used the five as fixtures use the samples or pinned fixtures instead: the host machinery they check — the clipboard history recording from the first start, the Files search over the file index, the root search's providers, the default extensions' tile icons, the memory peaks, the first-setup window — is the same through a sample that uses the same capability, installed as the default's identity where the test needs one. Behaviour that is an extension's own (the calculator's arithmetic, say) is its repository's to test, not Pane's.
- What told a contributor where the defaults live (the guests tree's README) points at their repositories now, and says how to work on them.

## Acceptance criteria

- [ ] No source of the five default extensions remains in Pane's repository; the samples, fixtures and helpers stay
- [ ] Every test that installed a default extension as a fixture installs a sample or pinned fixture instead, and still checks the host machinery it checked
- [ ] The guest build and the packaging produce no default-extension output
- [ ] Nothing references the removed folders: no build list, no workflow, no script, no doc
- [ ] CI: the `ci-fast.yml` verify run is green on the ticket branch

## Blocked by

- #284 — Pane's smokes install the defaults from their pinned repositories served on this computer
