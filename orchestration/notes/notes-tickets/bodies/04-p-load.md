## Parent

https://github.com/pane-app/pane/issues/123 (Launcher polish; ADR 0035)

## What to build

A one-pixel line along the rule under the search field shows a soft highlight sweeping across it (one pass every 1.5 seconds) while work the user is waiting for is pending — an invoked action or command call, an opened command's search, or root search waiting on its providers for the current query — but only once that work has lasted 300 ms, fading in and out over about 300 ms; work that ends sooner shows nothing. It replaces the footer's "Running…" text for such work. Background work Pane describes in words (a development build, acquisition, updates) keeps its words in the footer. The core exposes whether waited-for work is pending and since when; the window owns the drawing and the threshold. Under reduced motion the line shows at partial strength without sweeping. The busy state is announced through the existing status announcement only once the 300 ms threshold has passed, so quick actions are not announced as busy.

## Acceptance criteria

- [ ] An action that finishes in a few milliseconds never shows the bar and is never announced busy
- [ ] A slow action (the existing slow fixture) shows the bar after 300 ms, and it fades away when the work ends
- [ ] "Running…" no longer appears for such work; background progress text still does
- [ ] Under reduced motion the line is still, at partial strength
- [ ] The pending-since signal is exposed by the core and window tests run with controlled animation time
