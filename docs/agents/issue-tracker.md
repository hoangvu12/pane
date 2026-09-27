# Issue tracker: Local Markdown

Specs and issues live in `.scratch/` within this workspace.

## Conventions

- One feature per directory: `.scratch/<feature-slug>/`.
- Spec: `.scratch/<feature-slug>/spec.md`.
- Implementation tickets: one file per ticket at `.scratch/<feature-slug>/issues/<NN>-<slug>.md`, numbered from `01`.
- Triage status: a `Status:` line near the top, using the roles in `docs/agents/triage-labels.md`.
- Comments and conversation history: append under `## Comments`.

## Skill operations

When a skill says "publish to the issue tracker", write the corresponding local file. When it says "fetch the relevant ticket", read the referenced file. Resolve a ticket number within its feature directory; numbers are not globally unique.

Tickets produced by `/to-tickets` are already specified and do not need incoming-issue triage.

## Wayfinding operations

For `/wayfinder`, use:

- Map: `.scratch/<effort>/map.md`, containing Notes, Decisions-so-far and Fog.
- Child tickets: `.scratch/<effort>/issues/NN-<slug>.md`, with the question in the body and `Type: research|prototype|grilling|task`.
- Lifecycle: `Status: open`, `Status: claimed`, or `Status: resolved`. These are wayfinding lifecycle states, separate from triage roles.
- Blocking edges: `Blocked by: NN, NN`. A ticket is unblocked when every referenced ticket is resolved.
- Frontier: select the lowest-numbered open, unblocked ticket.
- Claim: set `Status: claimed` before working.
- Resolve: append `## Answer`, set `Status: resolved`, and add a short result plus a link to the map's Decisions-so-far.
