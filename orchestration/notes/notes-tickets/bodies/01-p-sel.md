## Parent

https://github.com/pane-app/pane/issues/123 (Launcher polish; ADR 0035)

## What to build

The selected row in every launcher surface — root search's result rows, a command's list, command search, confirmation rows, the Actions panel's entries and the Pane menu — is drawn as a wash of the text colour at about 10% alpha, with **no inset edge and no ring**, replacing the separate fixed washes those surfaces use today. The selected computed answer card loses its accent ring and shows the same wash. Where hovering does **not** move the selection (a surface whose hover is its own, such as pinned slots and footer buttons, and rows under an overlay that holds the target), a fainter wash of about 5% marks hover and fades out over about 70 ms when the pointer leaves; where hovering selects, as in the launcher's lists, the selection wash alone shows. The selection itself always changes at once, never fading. A control that takes the keyboard — a field, a Settings control, a focused slot — keeps its focus ring. Over a background image the selected row stays frosted as ADR 0028 draws it, without the edge. Under reduced motion there is no hover fade. The Settings window's sidebar and controls keep their own tokens.

## Acceptance criteria

- [ ] The selected row in root search, a command's list, command search, a confirmation and the Actions panel has no inset edge and shows the selection wash; one set of tokens serves them all
- [ ] A selected computed answer card shows no accent ring, only the wash
- [ ] A focused field or control keeps its focus ring
- [ ] Over a background image the selected row stays frosted, with no edge
- [ ] Where hovering does not select, the hover wash is lighter than the selection wash and fades out within its time; under reduced motion there is no fade
- [ ] Selection changes the moment a key is pressed; no fade on selection change
- [ ] Window tests in the launcher-window harness cover these cases, including reduced motion, in root search, a command list and the Actions panel
