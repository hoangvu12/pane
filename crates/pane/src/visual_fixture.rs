//! The Windows reference/native comparison workbench's native fixture (#91).
//!
//! This is the mechanism every visual ticket of the UI port (#90) uses: a
//! window that renders Pane's **production** GPUI components — the result
//! row, the keycap, the search header, the L1 panel and footer surfaces,
//! the footer action button — at the authored reference's client size,
//! with fixture data copied from the reference board, in named states a
//! capture helper drives. It is not a second UI: every visual value comes
//! from the same [`Theme`] tokens the launcher renders with, and the
//! components are the same functions the launcher calls. The composition
//! glue around them (which rows, which state, which keycaps) is this
//! harness's own, because the workbench owns scenario state, not the
//! launcher.
//!
//! The scenarios are the registration seam: a registry of [`Scenario`]s —
//! each a name, the client size it renders at, the rows it shows and the
//! [`Step`]s that drive it — that this ticket starts with the root/result
//! and keycap families. Later tickets register their own scenarios (the
//! contextual Actions panel, the calculator card, the clipboard split
//! view, the Appearance controls) by appending to [`scenarios`];
//! [`pending_scenarios`] records the reference boards whose native
//! scenarios do not exist yet, each with the ticket that will add them, so
//! the workbench reports them as pending rather than silently passing
//! them by. Store and the snap HUD stay source-only fixture references:
//! their boards are catalogued in the research, and no production or
//! fixture implementation is planned for them in this milestone.
//!
//! The steps are the one description of a scenario both capture helpers
//! follow — the native one against this window, the reference one against
//! the authored board — and [`replay`] replays them over the
//! fixture's own state model, so the manifest declares, for every capture,
//! which rows show, which is selected, which the pointer is over and where
//! each one lies. The comparison then validates each side against its own
//! declaration before comparing the sides with each other.
//!
//! What the fixture is **not**: it is not a parity test by itself. The
//! comparison that proves 1:1 fidelity lives in the workbench's scripts
//! (`scripts/visual-workbench/`). The fixture also never touches the
//! user's data: the runner gives it its own temporary data directory, it
//! loads no extensions and reads no installed commands, and the fixture
//! rows below are presentation values only — they never become the user's
//! installed-app list. The real launcher's own wiring is guarded
//! separately, by
//! `the_production_scenario_edits_searches_selects_opens_and_back_navigates`
//! in the window integration tests: fixture matching cannot hide a missing
//! production path.

use std::path::{Path, PathBuf};

use gpui::{
    App, Bounds, Context, Entity, FocusHandle, Focusable, FontWeight, Hsla, Pixels, Role,
    ScrollHandle, SharedString, TextRun, TitlebarOptions, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, size,
};
use gpui_elements::editable_text::{EditableTextState, StringStorage, TextChanged};
use pane_core::{
    Binding, KeyboardAction, ResultAction, ResultActionItem, ResultActions, SelectedAction,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::app::{KEY_CONTEXT, action_button};
use crate::features::actions_panel;
use crate::features::root_search::{self, search_header};
use crate::settings;
use crate::ui::footer;
use crate::ui::icon::{Glyph, IconTone, TileSize, tile_at};
use crate::ui::input::bind_text_editing;
use crate::ui::keycap::{self, CapMetrics, CapStyle};
use crate::ui::material::Material;
use crate::ui::result_row::{RowContent, RowMeta, result_row_with};
use crate::ui::shell::{self, LAUNCHER_CLIENT, SectionLabel};
use crate::ui::theme::Theme;
use crate::{Back, SelectNext, SelectPrevious};

/// The root reference board's client size, in logical pixels: the authored
/// 760×518 (64 search header + 404 list + 50 footer). Every root-family
/// fixture renders at exactly this client size — the launcher window's own
/// ([`LAUNCHER_CLIENT`]); the comparison never rescales an image to hide a
/// mismatch.
pub(crate) const ROOT_CLIENT: (f32, f32) = LAUNCHER_CLIENT;

/// A narrow launcher client: no reference board is this size. The launcher
/// window can be resized this small, and the narrow scenario shows that
/// the selected row and the footer stay visible and reachable there — an
/// adaptation, captured but never compared.
pub(crate) const NARROW_CLIENT: (f32, f32) = (480., 360.);

/// The Settings reference board's client size, for the fixture that
/// ticket #97 registers.
pub(crate) const SETTINGS_CLIENT: (f32, f32) = (1120., 720.);

/// The clipboard reference board's client size, for the fixture that
/// ticket #102 registers.
pub(crate) const CLIPBOARD_CLIENT: (f32, f32) = (940., 600.);

/// The keycap scenario's inset from the panel edge and the gap between
/// its caps — the harness's own layout, chosen so each cap stands alone
/// on the panel for measuring.
const KEYCAP_INSET: f32 = 20.;
const KEYCAP_GAP: f32 = 12.;

/// One fixture row: presentation values matching the authored reference
/// root board's own fixture data (its `Suggested` and `Commands` sections
/// at an empty query), so the native and reference captures show the
/// same content wherever the production components can represent it.
/// Kind, alias and shortcut metadata the reference also authors have no
/// production presentation yet — mismatches the comparison reports, not
/// ones this harness papers over. The icons are the reference's own tones
/// and glyphs, drawn by the production tile.
#[derive(Debug, PartialEq)]
pub(crate) struct FixtureRow {
    pub(crate) title: &'static str,
    pub(crate) subtitle: Option<&'static str>,
    pub(crate) unavailable: Option<&'static str>,
    pub(crate) icon: (IconTone, Glyph),
    /// The primary action's label the footer shows while the row is
    /// selected, as the reference board names it for the row's kind.
    pub(crate) action: &'static str,
    /// The row's kind, right-aligned ("Application", "Command").
    pub(crate) kind: &'static str,
    /// The alias the reference gives the row, if any.
    pub(crate) alias: Option<&'static str>,
    /// The hotkey the reference shows on the row, as a binding, if any.
    pub(crate) keys: Option<&'static str>,
}

impl FixtureRow {
    /// What the launcher's Actions panel lists for this row (see
    /// `pane_core::Launcher::result_actions`): its primary action, then,
    /// for a command — the fixture's commands stand in for installed ones
    /// — its hotkey and alias configuration, named by the core's rule for
    /// whether it has them.
    fn actions(&self) -> ResultActions {
        let mut items = vec![ResultActionItem {
            action: ResultAction::Invoke,
            label: self.action.to_owned(),
            available: self.unavailable.is_none(),
        }];
        if self.kind == "Command" {
            for (action, configured) in [
                (ResultAction::Hotkey, self.keys.is_some()),
                (ResultAction::Alias, self.alias.is_some()),
            ] {
                items.push(ResultActionItem {
                    action,
                    label: action
                        .configuration_label(configured)
                        .expect("a configuration entry")
                        .to_owned(),
                    available: true,
                });
            }
        }
        ResultActions {
            target: self.title.to_owned(),
            title: self.title.to_owned(),
            items,
        }
    }

    /// This row with the alias `alias`.
    const fn aliased(self, alias: &'static str) -> FixtureRow {
        FixtureRow {
            alias: Some(alias),
            ..self
        }
    }

    /// This row with the hotkey `binding`.
    const fn keyed(self, binding: &'static str) -> FixtureRow {
        FixtureRow {
            keys: Some(binding),
            ..self
        }
    }
}

/// An application row: the reference's `app()`, opened by "Open
/// Application".
const fn app(title: &'static str, icon: (IconTone, Glyph)) -> FixtureRow {
    FixtureRow {
        title,
        subtitle: None,
        unavailable: None,
        icon,
        action: "Open Application",
        kind: "Application",
        alias: None,
        keys: None,
    }
}

/// A command row: the reference's `cmd()`, run by "Run Command".
const fn command(
    title: &'static str,
    subtitle: &'static str,
    icon: (IconTone, Glyph),
) -> FixtureRow {
    FixtureRow {
        title,
        subtitle: Some(subtitle),
        unavailable: None,
        icon,
        action: "Run Command",
        kind: "Command",
        alias: None,
        keys: None,
    }
}

/// The reference root board's rows, in its order, with the kind, alias
/// and hotkey each carries there: its Suggested section's four (Figma,
/// Clipboard History, Left Half, Search Files) then its Commands
/// section's four (Plugin Store, Toggle Dark Mode, Lock Screen,
/// Settings). The fixture labels them as production does — one
/// "Commands" section for a blank query, never a claim of recent use
/// (#100) — so the reference's second label is a content difference the
/// comparison reports; its pinned strip above them is #101's.
pub(crate) const ROOT_ROWS: &[FixtureRow] = &[
    app("Figma", (IconTone::Pen, Glyph::Pen)),
    command(
        "Clipboard History",
        "Clipboard",
        (IconTone::Command, Glyph::Clipboard),
    )
    .aliased("cb")
    .keyed("ctrl-shift-v"),
    command(
        "Left Half",
        "Window Manager",
        (IconTone::Command, Glyph::Layout),
    )
    .keyed("win-alt-left"),
    command("Search Files", "Files", (IconTone::Command, Glyph::File)).aliased("f"),
    command("Plugin Store", "Pane", (IconTone::Command, Glyph::Blocks)).aliased("store"),
    command(
        "Toggle Dark Mode",
        "System",
        (IconTone::Command, Glyph::Moon),
    ),
    command("Lock Screen", "System", (IconTone::Command, Glyph::Lock)),
    command("Settings", "Pane", (IconTone::Command, Glyph::Sliders)).keyed("ctrl-,"),
];

/// A long-content row: the title and subtitle the production row's
/// truncation policy has to handle. The reference authors no long-content
/// row, so this state has no reference counterpart — it is a native-only
/// crop, recorded honestly as an adaptation rather than compared. The long
/// row carries an alias and keys too: the text truncates, its right-hand
/// parts never shrink.
pub(crate) const LONG_ROWS: &[FixtureRow] = &[
    command(
        "A result whose title runs far past the reference's own fixture rows and keeps going",
        "and whose subtitle is long enough to need the ellipsis the row owns",
        (IconTone::Command, Glyph::Prompt),
    )
    .aliased("long")
    .keyed("ctrl-shift-v"),
    command(
        "Left Half",
        "Window Manager",
        (IconTone::Command, Glyph::Layout),
    ),
];

/// A row that cannot run here, with the reason the production row shows.
/// The reference authors no unavailable state either: another native-only
/// adaptation, cropped and recorded as such. Its alias and keys stay
/// beside a reason long enough to wrap.
pub(crate) const UNAVAILABLE_ROWS: &[FixtureRow] = &[
    command(
        "Left Half",
        "Window Manager",
        (IconTone::Command, Glyph::Layout),
    ),
    FixtureRow {
        unavailable: Some(
            "The extension that provides this command is disabled; turn it back on in Settings to run it here",
        ),
        ..command(
            "Clipboard History",
            "Clipboard",
            (IconTone::Command, Glyph::Clipboard),
        )
        .aliased("cb")
        .keyed("ctrl-shift-v")
    },
];

/// The launcher's kind of a fixture row's kind label: an application's
/// primary action opens it (the Actions panel's arrow glyph).
fn row_kind(kind: &str) -> Option<pane_core::RowKind> {
    match kind {
        "Application" => Some(pane_core::RowKind::Application),
        "Command" => Some(pane_core::RowKind::Command),
        "File" => Some(pane_core::RowKind::File),
        "Fallback" => Some(pane_core::RowKind::Fallback),
        _ => None,
    }
}

/// The reference Actions board's rows: its "fig" query's four results
/// and its fallback, in its order, with Figma selected. The file row has
/// no tone of its own among the production tiles, so it shows the command
/// tile with the file glyph — a difference the comparison reports.
pub(crate) const ACTIONS_ROWS: &[FixtureRow] = &[
    app("Figma", (IconTone::Pen, Glyph::Pen)),
    command(
        "Recent Figma Files",
        "Design Files",
        (IconTone::Command, Glyph::File),
    ),
    FixtureRow {
        action: "Open File",
        kind: "File",
        ..command(
            "figma-tokens.json",
            "~/Design/tokens",
            (IconTone::Command, Glyph::File),
        )
    },
    command(
        "Configure Pane",
        "Settings",
        (IconTone::Command, Glyph::Sliders),
    ),
    FixtureRow {
        action: "Search Web",
        kind: "Fallback",
        ..command(
            "Search the web for “fig”",
            "Default browser",
            (IconTone::Command, Glyph::Globe),
        )
    },
];

/// Which component family a scenario renders.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Family {
    /// The root search composition: the search header, the result rows,
    /// the footer with its action button.
    Root,
    /// The keycap family, as the reference's Windows key groups pair
    /// against the production caps.
    Keycap,
    /// The icon tile family at each of its sizes.
    Tiles,
}

/// One step a capture helper takes, on either side. The helpers act with
/// real input — the native one through Windows' own pointer and keyboard,
/// the reference one through the browser's input events — never by
/// setting state behind the window's back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub(crate) enum Step {
    /// Save a capture of the client, named.
    Capture { name: &'static str },
    /// Move the pointer onto the center of the shown row at `row`, `nudge`
    /// pixels to the right of it, arriving from a pixel to its left (two
    /// moves, as a real pointer reports): a second move over the same row
    /// is movement only if it lands somewhere else.
    Pointer { row: usize, nudge: i16 },
    /// Press a key.
    Key { key: NamedKey },
    /// Type text into the focused field: the query, or the Actions
    /// panel's search while the panel is open.
    Type { text: &'static str },
    /// Click the element the fixture declares as `target` at its center
    /// (the footer's Actions button), the pointer moving there first.
    Click { target: &'static str },
}

/// The footer's Actions button, as a click step names it.
pub(crate) const ACTIONS_BUTTON: &str = "actions-button";

/// A click on `target`.
const fn click(target: &'static str) -> Step {
    Step::Click { target }
}

/// A key a step presses: the selection keys and Back under their default
/// bindings, which both capture helpers name the same way.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum NamedKey {
    Down,
    Escape,
}

const fn capture(name: &'static str) -> Step {
    Step::Capture { name }
}

/// The pointer onto the center of row `row`.
const fn pointer(row: usize) -> Step {
    Step::Pointer { row, nudge: 0 }
}

/// The selection key that moves to the next row.
const DOWN: Step = Step::Key {
    key: NamedKey::Down,
};

/// A scenario the workbench renders now, with production components.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Scenario {
    pub(crate) name: &'static str,
    pub(crate) description: &'static str,
    pub(crate) family: Family,
    /// The logical client size the fixture window must have for this
    /// scenario. The runner verifies the measured client against it.
    pub(crate) client: (f32, f32),
    /// Whether the authored reference has this state. A native-only
    /// scenario (an adaptation the reference never authors) is captured
    /// and kept as evidence, never compared and never counted as parity.
    pub(crate) reference: bool,
    /// The reference board the scenario pairs with, when it is not the
    /// interactive root board: a static board authors its one state, so
    /// the reference side captures it as authored and takes no steps.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) board: Option<&'static str>,
    /// The appearance the scenario renders in, when it is not the run's
    /// own (`--theme`): the light frame is captured in the same run as
    /// the dark ones. Light is a derived palette, so a light scenario is
    /// always native-only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) theme: Option<&'static str>,
    /// Whether the comparison measures the launcher's frame in this
    /// scenario's captures — the panel's inset edges, its corners, and
    /// where the header, list and footer begin and end (#92).
    pub(crate) frame: bool,
    #[serde(skip)]
    pub(crate) rows: &'static [FixtureRow],
    /// What the capture helpers do, in order, from the scenario's rest.
    pub(crate) steps: &'static [Step],
}

/// The scenarios this ticket starts the registry with: the root/result
/// family and the keycap family, over the production components that
/// exist today. Downstream tickets append theirs here — that is the
/// registration seam. Every scenario starts at rest (an empty, focused
/// query; the first row selected; the pointer outside the window) and
/// captures every state along its way, not only its endpoint.
pub(crate) fn scenarios() -> &'static [Scenario] {
    SCENARIOS
}

const SCENARIOS: &[Scenario] = {
    use Step::{Key, Type};
    &[
        Scenario {
            name: "root-rest",
            description: "The reference's empty query over its own fixture rows, row 0 selected",
            family: Family::Root,
            client: ROOT_CLIENT,
            reference: true,
            board: None,
            theme: None,
            frame: false,
            rows: ROOT_ROWS,
            steps: &[capture("rest")],
        },
        Scenario {
            name: "root-hover",
            description: "Rest, then the pointer moves onto unselected row 1, which selects it",
            family: Family::Root,
            client: ROOT_CLIENT,
            reference: true,
            board: None,
            theme: None,
            frame: false,
            rows: ROOT_ROWS,
            steps: &[capture("rest"), pointer(1), capture("hover")],
        },
        Scenario {
            name: "root-selected",
            description: "Rest, then the selection keys move the selection to row 2, one step at a time",
            family: Family::Root,
            client: ROOT_CLIENT,
            reference: true,
            board: None,
            theme: None,
            frame: false,
            rows: ROOT_ROWS,
            steps: &[
                capture("rest"),
                Key {
                    key: NamedKey::Down,
                },
                capture("down-1"),
                Key {
                    key: NamedKey::Down,
                },
                capture("selected"),
            ],
        },
        Scenario {
            name: "root-pointer-keys",
            description: "The pointer selects row 1; Down moves the selection to row 2 with the pointer resting on row 1, which keeps its hover wash; moving again over row 1 selects it",
            family: Family::Root,
            client: ROOT_CLIENT,
            reference: true,
            board: None,
            theme: None,
            frame: false,
            rows: ROOT_ROWS,
            steps: &[
                pointer(1),
                capture("pointed"),
                DOWN,
                capture("down-under-pointer"),
                Step::Pointer { row: 1, nudge: 6 },
                capture("moved-again"),
            ],
        },
        Scenario {
            name: "root-selected-hover",
            description: "Row 2 selected by the keys, then the pointer arrives on that selected row",
            family: Family::Root,
            client: ROOT_CLIENT,
            reference: true,
            board: None,
            theme: None,
            frame: false,
            rows: ROOT_ROWS,
            steps: &[
                Key {
                    key: NamedKey::Down,
                },
                Key {
                    key: NamedKey::Down,
                },
                capture("selected"),
                pointer(2),
                capture("selected-hover"),
            ],
        },
        Scenario {
            name: "root-focus",
            description: "Typing 'clip' into the focused field filters to the one matching row; Escape returns to rest",
            family: Family::Root,
            client: ROOT_CLIENT,
            reference: true,
            board: None,
            theme: None,
            frame: false,
            rows: ROOT_ROWS,
            steps: &[
                capture("rest"),
                Type { text: "clip" },
                capture("focus-typed"),
                Key {
                    key: NamedKey::Escape,
                },
                capture("back-to-rest"),
            ],
        },
        Scenario {
            name: "root-actions",
            description: "Down selects Clipboard History; the footer's Actions button opens its actions over the dimmed results; typing filters them to one, then to none; Escape closes the panel only",
            family: Family::Root,
            client: ROOT_CLIENT,
            reference: true,
            board: None,
            theme: None,
            frame: false,
            rows: ROOT_ROWS,
            steps: &[
                DOWN,
                capture("selected"),
                click(ACTIONS_BUTTON),
                capture("open"),
                Type { text: "alias" },
                capture("filtered"),
                Type { text: "zz" },
                capture("empty"),
                Key {
                    key: NamedKey::Escape,
                },
                capture("closed"),
            ],
        },
        Scenario {
            name: "actions-panel",
            description: "The Actions board: the query 'fig' with Figma selected and its actions open over the dimmed results",
            family: Family::Root,
            client: ROOT_CLIENT,
            reference: true,
            board: Some("actions"),
            theme: None,
            frame: false,
            rows: ACTIONS_ROWS,
            steps: &[Type { text: "fig" }, click(ACTIONS_BUTTON), capture("open")],
        },
        Scenario {
            name: "root-unavailable",
            description: "A row that cannot run here, with its reason (no reference counterpart)",
            family: Family::Root,
            client: ROOT_CLIENT,
            reference: false,
            board: None,
            theme: None,
            frame: false,
            rows: UNAVAILABLE_ROWS,
            steps: &[
                capture("rest"),
                Key {
                    key: NamedKey::Down,
                },
                capture("unavailable-selected"),
            ],
        },
        Scenario {
            name: "root-long-content",
            description: "A long title and subtitle under the row's truncation (no reference counterpart)",
            family: Family::Root,
            client: ROOT_CLIENT,
            reference: false,
            board: None,
            theme: None,
            frame: false,
            rows: LONG_ROWS,
            steps: &[capture("long-content")],
        },
        Scenario {
            name: "launcher-frame",
            description: "The launcher's frame at rest: the 760x518 panel, its inset edges and corners, and the header, list and footer boundaries",
            family: Family::Root,
            client: ROOT_CLIENT,
            reference: true,
            board: None,
            theme: None,
            frame: true,
            rows: ROOT_ROWS,
            steps: &[capture("frame")],
        },
        Scenario {
            name: "launcher-frame-light",
            description: "The same frame in the derived light appearance, at the same dimensions (no reference counterpart)",
            family: Family::Root,
            client: ROOT_CLIENT,
            reference: false,
            board: None,
            theme: Some("light"),
            frame: true,
            rows: ROOT_ROWS,
            steps: &[capture("frame-light")],
        },
        Scenario {
            name: "launcher-frame-narrow",
            description: "A narrow launcher: the keys select the last row, which the list scrolls into view above the footer (no reference counterpart)",
            family: Family::Root,
            client: NARROW_CLIENT,
            reference: false,
            board: None,
            theme: None,
            frame: true,
            rows: ROOT_ROWS,
            steps: &[
                capture("narrow-rest"),
                DOWN,
                DOWN,
                DOWN,
                DOWN,
                DOWN,
                DOWN,
                DOWN,
                capture("narrow-last-selected"),
            ],
        },
        Scenario {
            name: "tile-sizes",
            description: "The icon tile at the row's, a pinned slot's and the Actions header's size, as an application and as a command (no reference counterpart side by side)",
            family: Family::Tiles,
            client: ROOT_CLIENT,
            reference: false,
            board: None,
            theme: None,
            frame: false,
            rows: &[],
            steps: &[capture("tiles")],
        },
        Scenario {
            name: "keycap-windows",
            description: "The production key sequences for the reference's Windows chords, its accent Enter and a compact pinned hint, and rebound chords the reference never shows",
            family: Family::Keycap,
            client: ROOT_CLIENT,
            reference: true,
            board: None,
            theme: None,
            frame: false,
            rows: &[],
            steps: &[capture("keycaps")],
        },
    ]
};

/// A reference board whose native scenarios do not exist yet, with the
/// ticket that will register them. Running one fails with this list, so
/// a missing scenario is never mistaken for a passing comparison.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PendingScenario {
    pub(crate) name: &'static str,
    /// The reference board the scenario captures.
    pub(crate) board: &'static str,
    pub(crate) ticket: &'static str,
    pub(crate) description: &'static str,
    pub(crate) client: (f32, f32),
}

/// The registered-but-pending scenarios: saved reference boards whose
/// native fixtures later tickets add. Store and the snap HUD are
/// deliberately absent — they stay source-only fixture references for
/// this milestone (see the #90 specification's deferred capabilities).
pub(crate) fn pending_scenarios() -> &'static [PendingScenario] {
    &[
        PendingScenario {
            name: "calculator-card",
            board: "calculator",
            ticket: "https://github.com/hoangvu12/pane/issues/96",
            description: "The computed-result card from the calculator board",
            client: ROOT_CLIENT,
        },
        PendingScenario {
            name: "empty-state",
            board: "empty",
            ticket: "https://github.com/hoangvu12/pane/issues/96",
            description: "The no-results notice and fallback rows from the empty board",
            client: ROOT_CLIENT,
        },
        PendingScenario {
            name: "settings-shell",
            board: "settings",
            ticket: "https://github.com/hoangvu12/pane/issues/97",
            description: "The Settings window shell and sidebar at the Settings board's size",
            client: SETTINGS_CLIENT,
        },
        PendingScenario {
            name: "appearance-page",
            board: "settings",
            ticket: "https://github.com/hoangvu12/pane/issues/98",
            description: "The Appearance page's controls and live preview",
            client: SETTINGS_CLIENT,
        },
        PendingScenario {
            name: "pinned-strip",
            board: "root",
            ticket: "https://github.com/hoangvu12/pane/issues/101",
            description: "The five pinned quick slots above the root list",
            client: ROOT_CLIENT,
        },
        PendingScenario {
            name: "clipboard-split",
            board: "clipboard",
            ticket: "https://github.com/hoangvu12/pane/issues/102",
            description: "The supported text clipboard history in the split view",
            client: CLIPBOARD_CLIENT,
        },
    ]
}

/// One key sequence the keycap scenario renders: a production binding,
/// drawn through the launcher's own adapter
/// ([`crate::keyboard::binding_keys`]) in a cap style, and the reference
/// key group it pairs with, if the reference shows one.
pub(crate) struct KeyGroup {
    pub(crate) binding: &'static str,
    pub(crate) group: Option<&'static str>,
    pub(crate) style: CapStyle,
}

/// The keycap scenario's key sequences: the reference board's own chords
/// with its `platform` set to Windows — the footer's Actions Ctrl K, the
/// Left Half row's Win Alt ←, the Clipboard History row's Ctrl Shift V,
/// the footer's accent Enter and the first pinned slot's compact Ctrl 1 —
/// each paired by name with the reference's key group, so the comparison
/// sets the same effective binding beside each other; then chords the
/// reference never shows, which the primary action and a hint display
/// once the Keyboard page rebinds them (Shift+Enter, Ctrl+Enter,
/// Ctrl+Shift+P): captured, never compared.
pub(crate) const KEY_GROUPS: &[KeyGroup] = &[
    KeyGroup {
        binding: "ctrl-k",
        group: Some("footer-actions"),
        style: CapStyle::Regular,
    },
    KeyGroup {
        binding: "win-alt-left",
        group: Some("left-half"),
        style: CapStyle::Regular,
    },
    KeyGroup {
        binding: "ctrl-shift-v",
        group: Some("clipboard-history"),
        style: CapStyle::Regular,
    },
    KeyGroup {
        binding: "enter",
        group: Some("footer-primary"),
        style: CapStyle::Accent,
    },
    KeyGroup {
        binding: "ctrl-1",
        group: Some("pinned-1"),
        style: CapStyle::Compact,
    },
    KeyGroup {
        binding: "shift-enter",
        group: None,
        style: CapStyle::Accent,
    },
    KeyGroup {
        binding: "ctrl-enter",
        group: None,
        style: CapStyle::Accent,
    },
    KeyGroup {
        binding: "ctrl-shift-p",
        group: None,
        style: CapStyle::Regular,
    },
];

/// The tiles the tile scenario renders, left to right: each size as an
/// application (the reference's pen tone) and as a command, both with the
/// pen glyph — the same path at the application's 2px stroke and the
/// command's 1.6, so the comparison can weigh one against the other.
const TILES: &[(TileSize, IconTone, Glyph)] = &[
    (TileSize::Row, IconTone::Pen, Glyph::Pen),
    (TileSize::Row, IconTone::Command, Glyph::Pen),
    (TileSize::Slot, IconTone::Pen, Glyph::Pen),
    (TileSize::Slot, IconTone::Command, Glyph::Pen),
    (TileSize::Mini, IconTone::Pen, Glyph::Pen),
    (TileSize::Mini, IconTone::Command, Glyph::Pen),
];

/// The gap between the tile scenario's tiles.
const TILE_GAP: f32 = 24.;

/// A deliberate fault the workbench injects to prove its comparison is
/// sensitive to exactly the errors the port cares about. The perturbed
/// fixture renders with one visual value wrong; the comparison must fail,
/// on the check that value drives, by the amount injected. The manifest
/// keeps declaring the unperturbed values — the perturbation is a fault
/// in the rendering, not a change of intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Perturbation {
    /// The rows' horizontal inset grows by 4 logical pixels — the result
    /// list's side padding, an edge error every wash, tile and text
    /// position shifts by. (The name predates the list's own padding
    /// token; the runner keeps passing it.)
    RowPaddingPlus4,
    /// The selected row's wash is filled with a wrong color — a regional
    /// fill error the wash's measured color reports.
    SelectedFill,
    /// The hover wash is filled with a wrong color, the same kind of fault
    /// on the pointer's state.
    HoverFill,
}

/// Every perturbation, by the name the runner passes.
const PERTURBATIONS: &[(&str, Perturbation)] = &[
    ("row-padding-plus-4", Perturbation::RowPaddingPlus4),
    ("selected-fill", Perturbation::SelectedFill),
    ("hover-fill", Perturbation::HoverFill),
];

impl Perturbation {
    /// The perturbation `name` names, as the runner passes it.
    pub fn parse(name: &str) -> Result<Option<Perturbation>, String> {
        if name.is_empty() || name == "none" {
            return Ok(None);
        }
        PERTURBATIONS
            .iter()
            .find(|(known, _)| *known == name)
            .map(|&(_, perturbation)| Some(perturbation))
            .ok_or_else(|| {
                let known: Vec<_> = PERTURBATIONS.iter().map(|(known, _)| *known).collect();
                format!(
                    "unknown perturbation {name:?}; expected none or one of {}",
                    known.join(", ")
                )
            })
    }

    fn name(self) -> &'static str {
        PERTURBATIONS
            .iter()
            .find(|(_, perturbation)| *perturbation == self)
            .map(|(name, _)| *name)
            .expect("every perturbation is named")
    }

    /// `theme` with this fault applied: the frame renders a deliberately
    /// wrong value through the same production components.
    fn apply(self, theme: &mut Theme) {
        match self {
            Perturbation::RowPaddingPlus4 => theme.geometry.list_padding_x += px(4.),
            // A selected wash at white 25%: clearly wrong against the
            // authored 8.5%, and flat, so the measured fill is
            // deterministic in the opaque material.
            Perturbation::SelectedFill => {
                theme.row_selected = gpui::rgb_to_hsla(gpui::rgba(0xFFFFFF40))
            }
            // A hover wash at white 20% against the authored 3.5%.
            Perturbation::HoverFill => theme.row_hover = gpui::rgb_to_hsla(gpui::rgba(0xFFFFFF33)),
        }
    }
}

/// How the fixture runs: the scenario to render, its isolated data
/// directory (or [`None`] to keep settings in memory), the theme and
/// material to render with (as `PANE_THEME`/`PANE_MATERIAL` name them),
/// the perturbation to inject, and where the manifest is written.
#[derive(Debug, PartialEq)]
pub struct FixtureOptions {
    pub scenario: String,
    pub data_dir: Option<PathBuf>,
    pub theme: Option<String>,
    pub material: Option<String>,
    pub perturbation: Option<Perturbation>,
    pub manifest: PathBuf,
}

/// What the fixture binary was asked to do.
#[derive(Debug, PartialEq)]
pub enum Command {
    /// Open the fixture window for a scenario.
    Run(FixtureOptions),
    /// Write the scenario registry (active and pending) as JSON to a
    /// file and exit: the capture helpers read the steps from it.
    Registry(PathBuf),
}

const USAGE: &str = "usage: pane-visual-fixture --scenario <name> --manifest <file> \
    [--data-dir <dir>] [--theme dark|light] [--material glass|opaque] \
    [--perturb none|row-padding-plus-4|selected-fill|hover-fill]\n       \
    pane-visual-fixture --registry <file>";

/// Parses the fixture binary's arguments (without the program name).
pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut args = args.into_iter();
    let mut scenario = None;
    let mut manifest = None;
    let mut data_dir = None;
    let mut theme = None;
    let mut material = None;
    let mut perturbation = None;
    let mut registry = None;
    while let Some(flag) = args.next() {
        let mut value = || {
            args.next()
                .ok_or_else(|| format!("{flag} needs a value\n{USAGE}"))
        };
        match flag.as_str() {
            "--scenario" => scenario = Some(value()?),
            "--manifest" => manifest = Some(PathBuf::from(value()?)),
            "--data-dir" => data_dir = Some(PathBuf::from(value()?)),
            "--theme" => theme = Some(value()?),
            "--material" => material = Some(value()?),
            "--perturb" => perturbation = Perturbation::parse(&value()?)?,
            "--registry" => registry = Some(PathBuf::from(value()?)),
            other => return Err(format!("unknown argument {other:?}\n{USAGE}")),
        }
    }
    if let Some(registry) = registry {
        return Ok(Command::Registry(registry));
    }
    let scenario = scenario.ok_or_else(|| format!("--scenario is required\n{USAGE}"))?;
    let manifest = manifest.ok_or_else(|| format!("--manifest is required\n{USAGE}"))?;
    Ok(Command::Run(FixtureOptions {
        scenario,
        data_dir,
        theme,
        material,
        perturbation,
        manifest,
    }))
}

/// Runs what `command` asks for.
pub fn execute(command: Command) -> Result<(), String> {
    match command {
        Command::Registry(path) => write_registry(&path),
        Command::Run(options) => run(options),
    }
}

/// The scenario `name`, or why there is none: a pending scenario names
/// the ticket that will register it.
fn find_scenario(name: &str) -> Result<&'static Scenario, String> {
    if let Some(scenario) = scenarios().iter().find(|scenario| scenario.name == name) {
        return Ok(scenario);
    }
    if let Some(pending) = pending_scenarios()
        .iter()
        .find(|pending| pending.name == name)
    {
        return Err(format!(
            "scenario {name:?} is pending: {} registers it ({})",
            pending.ticket, pending.description
        ));
    }
    Err(format!(
        "unknown scenario {name:?}; the workbench renders one of: {}",
        scenarios()
            .iter()
            .map(|scenario| scenario.name)
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Registry {
    kind: &'static str,
    scenarios: &'static [Scenario],
    pending: &'static [PendingScenario],
}

fn write_registry(path: &Path) -> Result<(), String> {
    let registry = Registry {
        kind: "pane-visual-fixture-registry",
        scenarios: scenarios(),
        pending: pending_scenarios(),
    };
    write_json(path, &registry)
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|error| format!("could not serialize {}: {error}", path.display()))?;
    std::fs::write(path, json)
        .map_err(|error| format!("could not write {}: {error}", path.display()))
}

/// The fixture's state: the rows the query shows and the selection. The
/// window drives it from real input; [`replay`] drives it from
/// a scenario's steps — the same transitions, so what the manifest
/// declares for a capture is what the window shows when it is taken.
#[derive(Clone, Debug)]
struct FixtureState {
    all: &'static [FixtureRow],
    query: String,
    rows: Vec<&'static FixtureRow>,
    selected: usize,
    /// The Actions panel while it is open.
    actions: Option<PanelState>,
}

/// The open Actions panel's state: its filter and its selected entry.
#[derive(Clone, Debug, Default)]
struct PanelState {
    query: String,
    selected: usize,
}

impl FixtureState {
    fn new(all: &'static [FixtureRow]) -> FixtureState {
        FixtureState {
            all,
            query: String::new(),
            rows: all.iter().collect(),
            selected: 0,
            actions: None,
        }
    }

    /// The footer's Actions button, as the launcher's opens and closes
    /// its panel.
    fn toggle_actions(&mut self) {
        self.actions = match self.actions {
            Some(_) => None,
            None => Some(PanelState::default()),
        };
    }

    /// The selected row's actions, or `None` with nothing selected.
    fn target_actions(&self) -> Option<ResultActions> {
        self.rows.get(self.selected).map(|row| row.actions())
    }

    /// What the open panel lists now: the target's actions its filter
    /// keeps, by the core's own rule.
    fn listed_actions(&self) -> Vec<ResultActionItem> {
        let (Some(panel), Some(actions)) = (&self.actions, self.target_actions()) else {
            return Vec::new();
        };
        actions
            .matching(&panel.query)
            .into_iter()
            .cloned()
            .collect()
    }

    /// Text typed into the focused field: the panel's search while the
    /// panel is open, else the query.
    fn type_text(&mut self, text: &str) {
        match self.actions.as_mut() {
            Some(panel) => {
                panel.query.push_str(text);
                panel.selected = 0;
            }
            None => {
                let query = format!("{}{text}", self.query);
                self.set_query(&query);
            }
        }
    }

    /// Down: through the panel's entries that can run while it is open,
    /// as the launcher's panel moves, else the results.
    fn down(&mut self) {
        let listed = self.listed_actions();
        match self.actions.as_mut() {
            Some(panel) => {
                if let Some(next) = actions_panel::next_available(&listed, panel.selected, true) {
                    panel.selected = next;
                }
            }
            None => self.select_next(),
        }
    }

    /// Escape: it closes the panel while the panel is open, and only
    /// that; else the launcher's Back.
    fn escape(&mut self) {
        if self.actions.take().is_none() {
            self.back();
        }
    }

    /// The reference's own filtering rule, over the fixture's own data: a
    /// row shows when its title contains the query, ignoring case; a new
    /// query selects its first result, as the launcher's does.
    fn set_query(&mut self, query: &str) {
        let needle = query.to_lowercase();
        self.query = query.to_owned();
        self.rows = self
            .all
            .iter()
            .filter(|row| row.title.to_lowercase().contains(&needle))
            .collect();
        self.selected = 0;
    }

    fn select_next(&mut self) {
        if self.selected + 1 < self.rows.len() {
            self.selected += 1;
        }
    }

    fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// Escape, as root search's own Back behaves on a query: the query
    /// clears and the selection returns to the first row.
    fn back(&mut self) {
        self.set_query("");
    }

    /// The pointer moved over the shown row `row`: root search's rows
    /// select under the moving pointer (#94).
    fn pointer_over(&mut self, row: usize) {
        if row < self.rows.len() {
            self.selected = row;
        }
    }

    /// The section labels over the shown rows, by root search's own rule
    /// (`pane_core::root_sections`). The fixture lists no fallbacks.
    fn sections(&self) -> Vec<SectionLabel> {
        let rows = self.rows.len();
        let fallbacks = self
            .rows
            .iter()
            .position(|row| row.kind == "Fallback")
            .unwrap_or(rows);
        pane_core::root_sections(&self.query, rows, fallbacks)
            .iter()
            .map(SectionLabel::from)
            .collect()
    }
}

/// A rectangle in logical client coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub(crate) struct Rect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Rect {
    fn center(&self) -> (f32, f32) {
        (self.x + self.width / 2., self.y + self.height / 2.)
    }

    fn contains(&self, (x, y): (f32, f32)) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

/// The geometry the root composition declares in a client of `client`
/// size: the search header, the list's viewport and the footer, edge to
/// edge — the panel's inset ring takes no layout space.
struct Frame {
    search: Rect,
    list: Rect,
    footer: Rect,
}

fn frame(theme: &Theme, client: (f32, f32)) -> Frame {
    let geometry = &theme.geometry;
    let (width, height) = client;
    let search = Rect {
        x: 0.,
        y: 0.,
        width,
        height: f32::from(geometry.search_height),
    };
    let footer = Rect {
        x: 0.,
        y: height - f32::from(geometry.footer_height),
        width,
        height: f32::from(geometry.footer_height),
    };
    let list = Rect {
        x: 0.,
        y: search.y + search.height,
        width,
        height: footer.y - (search.y + search.height),
    };
    Frame {
        search,
        list,
        footer,
    }
}

/// One row as a capture declares it.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeclaredRow {
    title: &'static str,
    /// "app" for a gradient tile, "command" for the neutral one.
    tone: &'static str,
    subtitle: Option<&'static str>,
    unavailable: Option<&'static str>,
    selected: bool,
    hovered: bool,
    /// The row's rect. A row holding an unavailable reason grows past its
    /// height floor by the wrapped reason, which the fixture cannot
    /// declare without shaping text, so it and every row after it declare
    /// `heightIsFloor` and only the first such row's top is exact.
    rect: Rect,
    height_is_floor: bool,
    /// Whether the whole row lies inside the list's viewport at the
    /// list's scroll offset; a row scrolled out of view, or partly out,
    /// is declared but not measured.
    visible: bool,
    kind: &'static str,
    alias: Option<&'static str>,
    keys: Option<&'static str>,
    /// Where the query matched the title (byte ranges), by the launcher's
    /// own rule (`pane_core::title_matches`).
    matched: Vec<std::ops::Range<usize>>,
    /// Where the row's trailing parts lie, filled in from shaped text
    /// when the manifest is written.
    #[serde(skip_serializing_if = "Option::is_none")]
    kind_rect: Option<Rect>,
    #[serde(skip_serializing_if = "Option::is_none")]
    alias_rect: Option<Rect>,
    #[serde(skip_serializing_if = "Option::is_none")]
    key_group: Option<KeyGroupRecord>,
}

/// A section label as a capture declares it.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeclaredSection {
    label: String,
    note: Option<String>,
    rect: Rect,
}

/// The rows `state` shows, laid out in the list of `frame` scrolled by
/// `offset` (0 or negative, as GPUI's scroll offset is), with the pointer
/// at `pointer` (client coordinates) if it is in the window.
fn declared_rows(
    state: &FixtureState,
    theme: &Theme,
    frame: &Frame,
    offset: f32,
    pointer: Option<(f32, f32)>,
) -> Vec<DeclaredRow> {
    declared_list(state, theme, frame, offset, pointer).0
}

/// The rows and the section labels `state` shows, laid out down the list
/// (see [`declared_rows`]): each label ahead of its first row, a label's
/// height and the list's gap before it.
fn declared_list(
    state: &FixtureState,
    theme: &Theme,
    frame: &Frame,
    offset: f32,
    pointer: Option<(f32, f32)>,
) -> (Vec<DeclaredRow>, Vec<DeclaredSection>) {
    let geometry = &theme.geometry;
    let x = frame.list.x + f32::from(geometry.list_padding_x);
    let width = frame.list.width - 2. * f32::from(geometry.list_padding_x);
    let mut y = frame.list.y + f32::from(geometry.list_padding_top) + offset;
    let list = frame.list;
    let mut floor = false;
    let sections = state.sections();
    let mut labels = Vec::new();
    let rows = state
        .rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            for section in sections.iter().filter(|section| section.first == index) {
                labels.push(DeclaredSection {
                    label: section.label.to_string(),
                    note: section.note.as_ref().map(ToString::to_string),
                    rect: Rect {
                        x,
                        y,
                        width,
                        height: f32::from(geometry.section_height),
                    },
                });
                y += f32::from(geometry.section_height) + f32::from(geometry.row_list_gap);
            }
            floor |= row.unavailable.is_some();
            let rect = Rect {
                x,
                y,
                width,
                height: f32::from(geometry.row_min_height),
            };
            y += f32::from(geometry.row_min_height) + f32::from(geometry.row_list_gap);
            DeclaredRow {
                title: row.title,
                tone: if row.icon.0 == IconTone::Command {
                    "command"
                } else {
                    "app"
                },
                subtitle: row.subtitle,
                unavailable: row.unavailable,
                selected: index == state.selected,
                hovered: !floor && pointer.is_some_and(|point| rect.contains(point)),
                rect,
                height_is_floor: floor,
                visible: rect.y >= list.y && rect.y + rect.height <= list.y + list.height,
                kind: row.kind,
                alias: row.alias,
                keys: row.keys,
                matched: pane_core::title_matches(row.title, &state.query),
                kind_rect: None,
                alias_rect: None,
                key_group: None,
            }
        })
        .collect();
    (rows, labels)
}

/// One capture a scenario takes, as the fixture declares it.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeclaredCapture {
    name: &'static str,
    /// The steps taken since the scenario's rest, in order.
    after: Vec<Step>,
    query: String,
    /// The pointer's point, in logical client coordinates, while the
    /// capture is taken — the cursor's own glyph paints there, so the
    /// comparison masks a small region around it (never the control
    /// under test, which it measures beside the cursor).
    pointer: Option<(f32, f32)>,
    rows: Vec<DeclaredRow>,
    /// The section labels over the rows.
    sections: Vec<DeclaredSection>,
    /// The footer's primary action label: the selected row's.
    action: Option<&'static str>,
    /// The footer action button's rect, filled in from the shaped label
    /// when the manifest is written (the replay itself shapes no text).
    #[serde(skip_serializing_if = "Option::is_none")]
    action_button: Option<Rect>,
    /// Whether the Actions panel is open.
    actions_open: bool,
    /// The open Actions panel's layout.
    #[serde(skip_serializing_if = "Option::is_none")]
    actions: Option<DeclaredActions>,
    /// The footer's mark, hint and buttons, filled in from shaped text
    /// when the manifest is written.
    #[serde(skip_serializing_if = "Option::is_none")]
    footer: Option<DeclaredFooter>,
}

/// The open Actions panel as a capture declares it, laid out from the
/// panel's tokens (see [`declared_actions`]).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeclaredActions {
    /// The panel's own box.
    rect: Rect,
    /// The header, naming the target; none with nothing selected.
    #[serde(skip_serializing_if = "Option::is_none")]
    header: Option<DeclaredActionsHeader>,
    rows: Vec<DeclaredActionRow>,
    /// The group labels ("Pane").
    groups: Vec<DeclaredText>,
    /// The separators' 1px rules.
    rules: Vec<Rect>,
    /// The note shown in place of entries, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    empty: Option<DeclaredText>,
    /// The search row, its rule along its top edge.
    search: Rect,
    /// The dimmer over the results: the list's area.
    dimmer: Rect,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeclaredActionsHeader {
    title: String,
    rect: Rect,
    /// The target's 18px tile.
    tile: Rect,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeclaredActionRow {
    label: String,
    /// "invoke", "hotkey" or "alias".
    action: &'static str,
    rect: Rect,
    selected: bool,
    /// The entry's 16px glyph box.
    glyph: Rect,
}

/// A piece of text a capture declares: what it says and where its box is.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeclaredText {
    text: String,
    rect: Rect,
}

/// The footer as a capture declares it.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeclaredFooter {
    /// The Pane mark's 18px box.
    mark: Rect,
    /// The hint's parts, left to right.
    hint: Vec<DeclaredHintPart>,
    /// The rule between the buttons.
    divider: Rect,
    /// The Actions button, and whether it shows its panel open.
    actions_button: Rect,
    actions_pressed: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeclaredHintPart {
    /// The text, or the key sequence's caps' labels.
    text: String,
    keys: bool,
    /// Its horizontal extent, across the strip's first line.
    rect: Rect,
    /// Whether the part runs past the room the buttons leave the hint (a
    /// narrow window), where the hint is clipped.
    clipped: bool,
}

/// The open Actions panel's layout over `state`, as the production panel
/// draws it (see `crate::features::actions_panel`): its right edge 10px in
/// from the window's and its bottom 8px above the footer; inside, the
/// header, the list — entries, the group's rule and label unless the
/// filter narrows them, or the note — and the search row.
fn declared_actions(state: &FixtureState, theme: &Theme, frame: &Frame) -> Option<DeclaredActions> {
    let panel = state.actions.as_ref()?;
    let geometry = &theme.geometry.actions;
    let f = f32::from;
    let width = f(geometry.width);
    let x = frame.footer.x + frame.footer.width - f(geometry.inset) - width;
    let target = state.target_actions();
    let listed = state.listed_actions();
    let filtering = !panel.query.trim().is_empty();
    // Laid out from the panel's top at 0, then moved into place.
    let mut y = 0.;
    let header = target.as_ref().map(|actions| {
        let mini = f(theme.geometry.mini_tile.size);
        let content = f(geometry.header_height) - f(geometry.header_padding_top);
        DeclaredActionsHeader {
            title: actions.title.clone(),
            rect: Rect {
                x,
                y: 0.,
                width,
                height: f(geometry.header_height),
            },
            tile: Rect {
                x: x + f(geometry.header_padding_x),
                y: f(geometry.header_padding_top) + (content - mini) / 2.,
                width: mini,
                height: mini,
            },
        }
    });
    if header.is_some() {
        y += f(geometry.header_height);
    }
    y += f(geometry.list_padding);
    let inner_x = x + f(geometry.list_padding);
    let inner_width = width - 2. * f(geometry.list_padding);
    let gap = f(geometry.list_gap);
    // The list's children, top to bottom, with the gap between them.
    let mut children = 0;
    let mut child = |y: &mut f32| {
        if children > 0 {
            *y += gap;
        }
        children += 1;
    };
    let (mut rows, mut groups, mut rules) = (Vec::new(), Vec::new(), Vec::new());
    for panel_child in actions_panel::panel_children(&listed, filtering) {
        child(&mut y);
        match panel_child {
            actions_panel::PanelChild::Rule => {
                rules.push(Rect {
                    x: inner_x + f(geometry.rule_margin_x),
                    y: y + f(geometry.rule_margin_y),
                    width: inner_width - 2. * f(geometry.rule_margin_x),
                    height: 1.,
                });
                y += 1. + 2. * f(geometry.rule_margin_y);
            }
            actions_panel::PanelChild::Group => {
                groups.push(DeclaredText {
                    text: actions_panel::PANE_GROUP.to_owned(),
                    rect: Rect {
                        x: inner_x,
                        y,
                        width: inner_width,
                        height: f(geometry.group_height),
                    },
                });
                y += f(geometry.group_height);
            }
            actions_panel::PanelChild::Entry(index) => {
                let item = &listed[index];
                let glyph = f(geometry.glyph_size);
                rows.push(DeclaredActionRow {
                    label: item.label.clone(),
                    action: item.action.id(),
                    rect: Rect {
                        x: inner_x,
                        y,
                        width: inner_width,
                        height: f(geometry.row_height),
                    },
                    selected: index == panel.selected,
                    glyph: Rect {
                        x: inner_x + f(geometry.row_padding_x),
                        y: y + (f(geometry.row_height) - glyph) / 2.,
                        width: glyph,
                        height: glyph,
                    },
                });
                y += f(geometry.row_height);
            }
        }
    }
    let note = match (&target, listed.is_empty()) {
        (None, _) => Some(actions_panel::NOTHING_SELECTED),
        (Some(_), true) => Some(actions_panel::NO_MATCH),
        (Some(_), false) => None,
    };
    let empty = note.map(|text| {
        child(&mut y);
        let height = 2. * f(geometry.empty_padding_y)
            + f(theme.typography.action_size) * theme.typography.line_height;
        let rect = Rect {
            x: inner_x,
            y,
            width: inner_width,
            height,
        };
        y += height;
        DeclaredText {
            text: text.to_owned(),
            rect,
        }
    });
    y += f(geometry.list_padding);
    let search = Rect {
        x,
        y,
        width,
        height: f(geometry.search_height),
    };
    y += f(geometry.search_height);
    // Into place: the panel's bottom above the footer.
    let top = frame.footer.y - f(geometry.above_footer) - y;
    let place = |rect: Rect| Rect {
        y: rect.y + top,
        ..rect
    };
    Some(DeclaredActions {
        rect: place(Rect {
            x,
            y: 0.,
            width,
            height: y,
        }),
        header: header.map(|header| DeclaredActionsHeader {
            rect: place(header.rect),
            tile: place(header.tile),
            ..header
        }),
        rows: rows
            .into_iter()
            .map(|row| DeclaredActionRow {
                rect: place(row.rect),
                glyph: place(row.glyph),
                ..row
            })
            .collect(),
        groups: groups
            .into_iter()
            .map(|group| DeclaredText {
                rect: place(group.rect),
                ..group
            })
            .collect(),
        rules: rules.into_iter().map(place).collect(),
        empty: empty.map(|empty| DeclaredText {
            rect: place(empty.rect),
            ..empty
        }),
        search: place(search),
        dimmer: frame.list,
    })
}

/// A step with the point it acts at, resolved: the client point a
/// pointer step moves to, in logical client coordinates.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct ResolvedStep {
    #[serde(flatten)]
    step: Step,
    #[serde(skip_serializing_if = "Option::is_none")]
    point: Option<(f32, f32)>,
}

/// A scenario replayed over the fixture's state model: every capture it
/// takes and every step it takes, resolved.
pub(crate) struct Replay {
    pub(crate) captures: Vec<DeclaredCapture>,
    pub(crate) steps: Vec<ResolvedStep>,
}

/// The list's scroll offset after it scrolls the selected row into view
/// from `offset`, by the rule GPUI's `ScrollHandle::scroll_to_item`
/// applies (the least scroll that shows the whole row; the list's own
/// padding may scroll out of view), which both the launcher and the
/// fixture ask for whenever the selection moves.
fn scrolled_to_selected(state: &FixtureState, theme: &Theme, frame: &Frame, offset: f32) -> f32 {
    let rows = declared_rows(state, theme, frame, 0., None);
    let Some(row) = rows.get(state.selected) else {
        return offset;
    };
    let (top, bottom) = (frame.list.y, frame.list.y + frame.list.height);
    if row.rect.y + offset < top {
        top - row.rect.y
    } else if row.rect.y + row.rect.height + offset > bottom {
        bottom - (row.rect.y + row.rect.height)
    } else {
        offset
    }
}

/// Replays `scenario`'s steps over the fixture's state model: declares
/// every capture they take, and resolves where each pointer step points.
pub(crate) fn replay(scenario: &Scenario, theme: &Theme) -> Replay {
    let frame = frame(theme, scenario.client);
    let mut state = FixtureState::new(scenario.rows);
    let mut offset = 0.;
    let mut pointer = None;
    let mut after = Vec::new();
    let mut captures = Vec::new();
    let mut steps = Vec::new();
    for step in scenario.steps {
        let mut point = None;
        match *step {
            Step::Capture { name } => {
                let (rows, sections) = declared_list(&state, theme, &frame, offset, pointer);
                captures.push(DeclaredCapture {
                    name,
                    after: after.clone(),
                    query: state.query.clone(),
                    pointer,
                    rows,
                    sections,
                    action: state.rows.get(state.selected).map(|row| row.action),
                    action_button: None,
                    actions_open: state.actions.is_some(),
                    actions: declared_actions(&state, theme, &frame),
                    footer: None,
                })
            }
            Step::Pointer { row, nudge } => {
                let rows = declared_rows(&state, theme, &frame, offset, None);
                let moved_to = rows.get(row).map(|shown| {
                    let (x, y) = shown.rect.center();
                    (x + f32::from(nudge), y)
                });
                // The capture helpers move the pointer onto the row from a
                // pixel to its left, as a real pointer arrives: the first
                // event records where it is, the second is movement and
                // selects the row. A step that lands where the pointer
                // already is moves nothing.
                if moved_to.is_some() && moved_to != pointer {
                    state.pointer_over(row);
                }
                pointer = moved_to;
                point = pointer;
            }
            Step::Key {
                key: NamedKey::Down,
            } => state.down(),
            Step::Key {
                key: NamedKey::Escape,
            } => state.escape(),
            Step::Type { text } => state.type_text(text),
            // The point is the button's center, which shaping places: the
            // manifest fills it in.
            Step::Click { target } => {
                if target == ACTIONS_BUTTON {
                    state.toggle_actions();
                }
            }
        }
        if matches!(
            step,
            Step::Key { .. } | Step::Type { .. } | Step::Pointer { .. }
        ) {
            offset = scrolled_to_selected(&state, theme, &frame, offset);
        }
        if !matches!(step, Step::Capture { .. }) {
            after.push(*step);
        }
        steps.push(ResolvedStep { step: *step, point });
    }
    Replay { captures, steps }
}

/// The fixture window: the scenario's state over the production
/// components. It owns the query field's editable text entity (the same
/// element the launcher's fields use) and the fixture's state; the
/// selection keys, the typing and the pointer act on it exactly as the
/// capture helper drives them, through the production bindings the
/// launcher itself registers.
pub(crate) struct FixtureWindow {
    scenario: &'static Scenario,
    perturbation: Option<Perturbation>,
    state: FixtureState,
    query: Entity<EditableTextState>,
    /// The result list's scroll, kept on the selected row as the
    /// launcher keeps its own (see [`scrolled_to_selected`]).
    scroll: ScrollHandle,
    /// Where the pointer last moved, as the launcher keeps it: only real
    /// movement over a row selects it.
    pointer: Option<gpui::Point<Pixels>>,
    /// The Actions panel's search field (the panel's state is the
    /// fixture state's).
    filter: Entity<EditableTextState>,
}

impl FixtureWindow {
    fn new(
        scenario: &'static Scenario,
        perturbation: Option<Perturbation>,
        cx: &mut Context<Self>,
    ) -> FixtureWindow {
        let query = cx.new(|cx| EditableTextState::new(StringStorage::default(), cx));
        query.focus_handle(cx).tab_stop(true);
        cx.subscribe(&query, |this, input, _: &TextChanged, cx| {
            let text = input.read(cx).as_str().to_owned();
            if text != this.state.query {
                this.state.set_query(&text);
                this.selection_moved(cx);
            }
        })
        .detach();
        let filter = cx.new(|cx| EditableTextState::new(StringStorage::default(), cx));
        cx.subscribe(&filter, |this, input, _: &TextChanged, cx| {
            let text = input.read(cx).as_str().to_owned();
            if let Some(panel) = this.state.actions.as_mut()
                && panel.query != text
            {
                panel.query = text;
                panel.selected = 0;
                cx.notify();
            }
        })
        .detach();
        FixtureWindow {
            scenario,
            perturbation,
            state: FixtureState::new(scenario.rows),
            query,
            scroll: ScrollHandle::new(),
            pointer: None,
            filter,
        }
    }

    /// The footer's Actions button: opens the panel with focus in its
    /// search, or closes it and gives focus back to the query, as the
    /// launcher's does.
    fn toggle_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.state.toggle_actions();
        let focus = if self.state.actions.is_some() {
            self.filter.update(cx, |filter, cx| filter.emplace("", cx));
            self.filter.focus_handle(cx)
        } else {
            self.query.focus_handle(cx)
        };
        window.focus(&focus, cx);
        cx.notify();
    }

    fn close_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.actions.is_some() {
            self.toggle_actions(window, cx);
        }
    }

    fn actions_next(
        &mut self,
        _: &actions_panel::NextAction,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.down();
        cx.notify();
    }

    fn actions_previous(
        &mut self,
        _: &actions_panel::PreviousAction,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let listed = self.state.listed_actions();
        if let Some(panel) = self.state.actions.as_mut()
            && let Some(previous) = actions_panel::next_available(&listed, panel.selected, false)
        {
            panel.selected = previous;
            cx.notify();
        }
    }

    fn actions_close(
        &mut self,
        _: &actions_panel::CloseActions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_actions(window, cx);
    }

    /// The pointer moved over row `index` to `position`: real movement
    /// selects it, as the launcher's root search does, without scrolling
    /// the list (only the keys' selection scrolls).
    fn pointer_moved_over(
        &mut self,
        index: usize,
        position: gpui::Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let moved = self.pointer.is_some_and(|last| last != position);
        // The open panel holds its target, as the launcher's does.
        if moved && self.state.selected != index && self.state.actions.is_none() {
            self.state.pointer_over(index);
            cx.notify();
        }
    }

    /// After the selection may have moved: the list keeps the selected
    /// row in view, as the launcher's does, and the window redraws.
    fn selection_moved(&mut self, cx: &mut Context<Self>) {
        self.scroll.scroll_to_item(shell::child_of_row(
            &self.state.sections(),
            self.state.selected,
        ));
        cx.notify();
    }

    fn select_next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        self.state.select_next();
        self.selection_moved(cx);
    }

    fn select_previous(&mut self, _: &SelectPrevious, _: &mut Window, cx: &mut Context<Self>) {
        self.state.select_previous();
        self.selection_moved(cx);
    }

    /// The launcher's Back, under its effective binding: the query clears
    /// and the selection returns to the first row.
    fn back(&mut self, _: &Back, _: &mut Window, cx: &mut Context<Self>) {
        self.state.back();
        let query = self.query.clone();
        query.update(cx, |query, cx| query.emplace("", cx));
        self.selection_moved(cx);
    }

    /// The theme this frame renders with: the settings' theme, with the
    /// perturbation applied when the run injects one.
    fn theme(&self, cx: &App) -> Theme {
        let mut theme = settings::visuals(cx).theme;
        if let Some(perturbation) = self.perturbation {
            perturbation.apply(&mut theme);
        }
        theme
    }

    fn render_keycaps(&self, theme: &Theme) -> gpui::Div {
        let caps = KEY_GROUPS.iter().map(|group| {
            let keys = crate::keyboard::binding_keys(&parse_binding(group.binding));
            // Each group gets its own scope: the group's id is fixed.
            div()
                .id(SharedString::from(format!("keys-{}", group.binding)))
                .flex()
                .child(keycap::key_sequence(&keys, group.style, theme))
        });
        div()
            .key_context(KEY_CONTEXT)
            .size_full()
            .flex()
            .flex_col()
            .items_start()
            .p(px(KEYCAP_INSET))
            .gap(px(KEYCAP_GAP))
            .font_family(theme.typography.family.clone())
            .font_features(theme.typography.features.clone())
            .text_color(theme.text_title)
            .children(caps)
    }

    fn render_tiles(&self, theme: &Theme) -> gpui::Div {
        div()
            .size_full()
            .flex()
            .items_start()
            .p(px(KEYCAP_INSET))
            .gap(px(TILE_GAP))
            .children(
                TILES
                    .iter()
                    .map(|&(size, tone, glyph)| tile_at(size, tone, glyph, theme)),
            )
    }

    fn render_root(&self, theme: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let rows = self.state.rows.iter().enumerate().map(|(index, row)| {
            let selected = index == self.state.selected;
            result_row_with(
                RowContent {
                    title: row.title.into(),
                    subtitle: row.subtitle.map(Into::into),
                    unavailable_reason: row.unavailable.map(Into::into),
                    unavailable_id: ("unavailable", index).into(),
                    selected,
                    icon: Some(row.icon),
                },
                RowMeta {
                    matched: pane_core::title_matches(row.title, &self.state.query),
                    alias: row.alias.map(Into::into),
                    keys: row
                        .keys
                        .map(|binding| crate::keyboard::binding_keys(&parse_binding(binding))),
                    kind: Some(row.kind.into()),
                },
                theme,
            )
            .id(("row", index))
            .role(Role::ListBoxOption)
            .aria_label(row.title)
            .aria_selected(selected)
            .on_mouse_move(
                cx.listener(move |this, event: &gpui::MouseMoveEvent, _, cx| {
                    this.pointer_moved_over(index, event.position, cx);
                }),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                // A click selects the row it lands on; the fixture opens
                // nothing — there is no command behind fixture data.
                this.state.selected = index;
                cx.notify();
            }))
            .into_any_element()
        });
        // The launcher's own result list.
        let list = shell::result_list(theme)
            .aria_label("Results")
            .track_scroll(&self.scroll)
            .children(shell::with_section_labels(
                rows,
                &self.state.sections(),
                theme,
            ));
        let focus = self.query.focus_handle(cx);
        let search = div()
            .id("search")
            .key_context(root_search::search_context())
            .track_focus(&focus)
            .role(Role::EditableComboBox)
            .aria_label("Search")
            .flex_1()
            .min_h(px(0.))
            .flex()
            .flex_col()
            .child(search_header(
                &self.query,
                root_search::ROOT_PLACEHOLDER,
                theme,
            ))
            // The results, under the dimmer while the panel is open, as
            // the launcher composes them.
            .child(actions_panel::dimmed(
                list.into_any_element(),
                self.state.actions.is_some(),
                theme,
            ));
        // The footer, composed as the launcher's (`ui::footer`): the mark
        // (the launcher's app menu; the fixture opens no menu), the hint
        // and the buttons — the selected row's primary action, the rule
        // and Actions — and the open panel over the strip.
        let keyboard = settings::keyboard_of(cx);
        let invoke = keyboard
            .binding(KeyboardAction::InvokeSelectedAction)
            .clone();
        let invoke_keys = crate::keyboard::binding_keys(&invoke);
        let open_keys =
            crate::keyboard::binding_keys(keyboard.binding(KeyboardAction::OpenActions));
        // The selected row's primary action, as the reference's footer
        // follows the selection; no row, no button.
        let action = self
            .state
            .rows
            .get(self.state.selected)
            .map(|row| SelectedAction {
                label: row.action.into(),
                available: true,
            });
        let open = self.state.actions.is_some();
        let buttons = footer::buttons(
            action.map(|action| action_button(&action, &invoke, theme).into_any_element()),
            Some(
                footer::actions_button(&open_keys, open, theme)
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_actions(window, cx)))
                    .into_any_element(),
            ),
            theme,
        );
        let hint = footer::hint_line(
            footer::hint_parts(
                open,
                invoke_keys.clone(),
                open_keys,
                crate::keyboard::escape_keys(),
            ),
            theme,
        );
        let panel = self.state.actions.as_ref().map(|panel| {
            let target = self.state.target_actions();
            let kind = self
                .state
                .rows
                .get(self.state.selected)
                .and_then(|row| row_kind(row.kind));
            let listed = self.state.listed_actions();
            let surface = actions_panel::compose(
                actions_panel::PanelView {
                    target: target.as_ref().map(|actions| (actions, kind)),
                    icon: self.state.rows.get(self.state.selected).map(|row| row.icon),
                    listed: &listed,
                    filtering: !panel.query.trim().is_empty(),
                    selected: panel.selected,
                    invoke: &invoke_keys,
                    filter: &self.filter,
                },
                theme,
                settings::visuals(cx).material,
                |row, _| row,
            )
            .key_context(actions_panel::CONTEXT)
            .on_action(cx.listener(Self::actions_next))
            .on_action(cx.listener(Self::actions_previous))
            .on_action(cx.listener(Self::actions_close))
            .on_mouse_down_out(cx.listener(
                |this, _: &gpui::MouseDownEvent, window, cx| {
                    this.close_actions(window, cx);
                    cx.stop_propagation();
                },
            ));
            actions_panel::anchored(surface, theme)
        });
        let footer = Material::footer(theme)
            .id("status")
            .relative()
            .when_some(panel, |strip, panel| strip.child(panel))
            .text_size(theme.typography.footer_size)
            .child(footer::footer_row(
                footer::mark_button(theme).into_any_element(),
                footer::hint_slot(Some(hint), theme).into_any_element(),
                buttons,
                theme,
            ));
        div()
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::back))
            // After the rows' own handlers: a row compares the event with
            // the position before it.
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, _| {
                this.pointer = Some(event.position);
            }))
            .size_full()
            .flex()
            .flex_col()
            .font_family(theme.typography.family.clone())
            .font_features(theme.typography.features.clone())
            .text_color(theme.text_title)
            .child(search)
            .child(footer)
    }
}

impl Focusable for FixtureWindow {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.query.focus_handle(cx)
    }
}

impl Render for FixtureWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let material = settings::visuals(cx).material;
        let theme = self.theme(cx);
        let content = match self.scenario.family {
            Family::Keycap => self.render_keycaps(&theme),
            Family::Tiles => self.render_tiles(&theme),
            Family::Root => self.render_root(&theme, cx),
        };
        material.panel(&theme, content)
    }
}

/// The fixture's manifest: the declared values the comparison measures
/// against, from what the fixture actually rendered with — the theme the
/// settings resolved, the layout the composition lays out, the bindings
/// in force, the label widths the text system shaped — in logical client
/// coordinates, with the window's own bounds and scale.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    kind: &'static str,
    scenario: &'static Scenario,
    perturbation: Option<&'static str>,
    /// The panel material and theme in force (the comparison reads
    /// colors strictly only in the opaque material).
    material: &'static str,
    appearance: &'static str,
    window: WindowRecord,
    declared: DeclaredTokens,
    search_header: Rect,
    list: Rect,
    footer: Rect,
    captures: Vec<DeclaredCapture>,
    /// The scenario's steps, with each pointer step's client point.
    steps: Vec<ResolvedStep>,
    keycaps: Vec<KeyGroupRecord>,
    tiles: Vec<TileRecord>,
    fonts: Vec<FontRecord>,
    /// How the text system resolved each embedded face (see
    /// [`FontResolution`]).
    font_resolution: Vec<FontResolution>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WindowRecord {
    /// [`Window::bounds`]: origin and logical size, as GPUI reports them.
    bounds: [f32; 4],
    /// [`Window::viewport_size`]: the logical client the fixture paints.
    viewport: [f32; 2],
    scale_factor: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeclaredTokens {
    geometry: GeometryRecord,
    colors: ColorRecord,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeometryRecord {
    search_height: f32,
    search_padding_x: f32,
    search_gap: f32,
    search_size: f32,
    row_min_height: f32,
    row_radius: f32,
    row_padding_x: f32,
    row_gap: f32,
    row_list_gap: f32,
    row_title_size: f32,
    row_subtitle_size: f32,
    list_padding_top: f32,
    list_padding_bottom: f32,
    list_padding_x: f32,
    tile_size: f32,
    tile_radius: f32,
    footer_height: f32,
    footer_padding_left: f32,
    footer_padding_right: f32,
    action_height: f32,
    action_radius: f32,
    action_padding_x: f32,
    action_gap: f32,
    keycap_height: f32,
    keycap_radius: f32,
    keycap_padding_x: f32,
    keycap_compact_height: f32,
    keycap_compact_padding_x: f32,
    key_gap: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ColorRecord {
    panel_solid: Hex,
    keycap_background: Hex,
    keycap_bottom: Hex,
    alias_text: Hex,
    keycap_text: Hex,
    accent: Hex,
    accent_ink: Hex,
    panel_top_highlight: Hex,
    row_hover: Hex,
    row_selected: Hex,
    row_selected_border: Hex,
    footer_tint: Hex,
    hairline: Hex,
    hairline_soft: Hex,
    tile_background: Hex,
    tile_foreground: Hex,
    text_title: Hex,
    text_muted: Hex,
    text_query: Hex,
    text_placeholder: Hex,
    popover_solid: Hex,
    popover_edge: Hex,
    footer_mark: Hex,
    footer_button_text: Hex,
    control_hover: Hex,
    footer_button_open: Hex,
    footer_divider: Hex,
    action_selected: Hex,
    action_text: Hex,
    action_icon: Hex,
    action_rule: Hex,
    actions_dimmer: Hex,
}

/// A color as the manifest writes it: `#RRGGBBAA`.
struct Hex(Hsla);

impl Serialize for Hex {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&hex(self.0))
    }
}

fn hex(color: Hsla) -> String {
    let rgba = gpui::hsla_to_rgba(color);
    let channel = |value: f32| (value.clamp(0., 1.) * 255.).round() as u8;
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        channel(rgba.color.red),
        channel(rgba.color.green),
        channel(rgba.color.blue),
        channel(rgba.alpha)
    )
}

/// One key sequence as the manifest declares it: its caps, where each
/// lies, and the type they are set in.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct KeyGroupRecord {
    /// The binding the sequence shows, in the record's grammar.
    binding: &'static str,
    /// The reference key group it pairs with, if any.
    group: Option<&'static str>,
    style: &'static str,
    /// The sequence's accessible name.
    name: String,
    /// The caps' type: "11px Geist Mono 500".
    font: String,
    rect: Rect,
    caps: Vec<CapRecord>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CapRecord {
    /// The cap's label, as both sides are controlled to show it.
    label: String,
    /// The label's shaped width at the cap's font, size and weight.
    label_width: f32,
    rect: Rect,
}

/// One tile of the tile scenario.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TileRecord {
    size: &'static str,
    /// "app" (a gradient tone, 2px glyph) or "command" (the neutral tile).
    tone: &'static str,
    rect: Rect,
    glyph: f32,
}

/// How the text system resolved one of the embedded faces: a face the
/// text system could not find resolves to its fallback — the same face as
/// the probe for a family no system has — so a missing Geist is visible
/// in the manifest rather than silently drawn in another font.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FontResolution {
    family: SharedString,
    weight: f32,
    font_id: usize,
    /// The shaped width of [`FONT_PROBE`] at 14px in the face.
    probe_width: f32,
}

/// The text the font resolution check shapes in every face.
const FONT_PROBE: &str = "Geist Ctrl+Shift+V";

/// A family no system has: it resolves to the text system's fallback.
const MISSING_FONT: &str = "Pane Missing Font Probe";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FontRecord {
    file: &'static str,
    bytes: usize,
    sha256: String,
}

/// The width `text` shapes to in `theme`'s family at `size` and `weight`.
fn shaped_width(
    window: &Window,
    theme: &Theme,
    text: &str,
    size: Pixels,
    weight: FontWeight,
) -> f32 {
    shaped_width_in(
        window,
        theme.typography.family.clone(),
        theme,
        text,
        size,
        weight,
    )
}

/// The width `text` shapes to in `family` at `size` and `weight`.
fn shaped_width_in(
    window: &Window,
    family: SharedString,
    theme: &Theme,
    text: &str,
    size: Pixels,
    weight: FontWeight,
) -> f32 {
    let mut font = gpui::font(family);
    font.weight = weight;
    // The features every text run asks for (kerning), as rendered.
    font.features = theme.typography.features.clone();
    let run = TextRun {
        len: text.len(),
        font,
        color: theme.text_title,
        background_color: None,
        underline: None,
        strikethrough: None,
        letter_spacing: None,
    };
    window
        .text_system()
        .shape_line(SharedString::from(text.to_owned()), size, &[run], None)
        .width()
        .into()
}

/// Where `row`'s trailing parts lie, right-aligned inside its padding
/// with the row's gap between them, as the shared row lays them out: the
/// kind (at least its least width, its text right-aligned), the key
/// sequence, then the alias chip.
fn declare_trailing(window: &Window, theme: &Theme, row: &mut DeclaredRow) {
    let geometry = &theme.geometry;
    let typography = &theme.typography;
    let gap = f32::from(geometry.row_gap);
    let center = row.rect.y + row.rect.height / 2.;
    let mut right = row.rect.x + row.rect.width - f32::from(geometry.row_padding_x);
    let kind_width = shaped_width(
        window,
        theme,
        row.kind,
        typography.row_kind_size,
        typography.regular,
    )
    .max(f32::from(geometry.row_kind_min_width));
    row.kind_rect = Some(Rect {
        x: right - kind_width,
        y: row.rect.y,
        width: kind_width,
        height: row.rect.height,
    });
    right -= kind_width + gap;
    if let Some(binding) = row.keys {
        let height = f32::from(geometry.keycap_height);
        let mut group = declared_group(
            window,
            theme,
            (binding, &parse_binding(binding)),
            None,
            CapStyle::Regular,
            (0., center - height / 2.),
        );
        let shift = right - group.rect.width;
        group.rect.x += shift;
        for cap in &mut group.caps {
            cap.rect.x += shift;
        }
        right -= group.rect.width + gap;
        row.key_group = Some(group);
    }
    if let Some(alias) = row.alias {
        let width = shaped_width_in(
            window,
            typography.mono_family.clone(),
            theme,
            alias,
            typography.alias_size,
            FontWeight::NORMAL,
        ) + 2. * f32::from(geometry.alias_padding_x);
        row.alias_rect = Some(Rect {
            x: right - width,
            y: row.rect.y,
            width,
            height: row.rect.height,
        });
    }
}

fn parse_binding(binding: &str) -> Binding {
    Binding::parse(binding).unwrap_or_else(|why| panic!("keycap fixture binding {binding}: {why}"))
}

fn style_name(style: CapStyle) -> &'static str {
    match style {
        CapStyle::Regular => "regular",
        CapStyle::Compact => "compact",
        CapStyle::Accent => "accent",
    }
}

/// `binding`'s key sequence in `style`, laid out from `(x, y)` by the
/// rules [`keycap::key_sequence`] draws it with: each cap its label's
/// shaped width plus its padding, at least as wide as it is high, the
/// caps `key_gap` apart.
/// Each cap's label width and its own width, as the production keycap
/// lays `keys` out in `style`: the label's shaped width in Geist Mono
/// 500 and the cap's padding, at least as wide as the cap is tall.
fn cap_widths(
    window: &Window,
    theme: &Theme,
    keys: &keycap::KeySequence,
    style: CapStyle,
) -> Vec<(f32, f32)> {
    let CapMetrics {
        height,
        padding_x: padding,
        text_size: size,
    } = style.metrics(theme);
    keys.keys
        .iter()
        .map(|key| {
            let label_width = shaped_width_in(
                window,
                theme.typography.mono_family.clone(),
                theme,
                &key.cap,
                size,
                theme.typography.medium,
            );
            let width = (label_width + 2. * f32::from(padding)).max(f32::from(height));
            (label_width, width)
        })
        .collect()
}

/// The width `keys` lays out to in `style`: its caps and the gaps between
/// them.
fn keys_width(window: &Window, theme: &Theme, keys: &keycap::KeySequence, style: CapStyle) -> f32 {
    let widths = cap_widths(window, theme, keys, style);
    let gaps = widths.len().saturating_sub(1) as f32 * f32::from(theme.geometry.key_gap);
    widths.iter().map(|(_, width)| width).sum::<f32>() + gaps
}

fn declared_group(
    window: &Window,
    theme: &Theme,
    (id, binding): (&'static str, &Binding),
    group: Option<&'static str>,
    style: CapStyle,
    (x, y): (f32, f32),
) -> KeyGroupRecord {
    let geometry = &theme.geometry;
    let typography = &theme.typography;
    let CapMetrics {
        height,
        text_size: size,
        ..
    } = style.metrics(theme);
    let keys = crate::keyboard::binding_keys(binding);
    let mut left = x;
    let mut caps = Vec::new();
    for (key, (label_width, width)) in keys
        .keys
        .iter()
        .zip(cap_widths(window, theme, &keys, style))
    {
        caps.push(CapRecord {
            label: key.cap.to_string(),
            label_width,
            rect: Rect {
                x: left,
                y,
                width,
                height: f32::from(height),
            },
        });
        left += width + f32::from(geometry.key_gap);
    }
    let width = left - f32::from(geometry.key_gap) - x;
    KeyGroupRecord {
        binding: id,
        group,
        style: style_name(style),
        name: keys.name(),
        font: format!(
            "{}px {} {}",
            f32::from(size),
            typography.mono_family,
            typography.medium.0
        ),
        rect: Rect {
            x,
            y,
            width,
            height: f32::from(height),
        },
        caps,
    }
}

/// Every embedded face the theme names — Geist and Geist Mono at 400 and
/// 500 — as the text system resolves it, then a family no system has at
/// both weights (the fallback, for comparison).
fn font_resolution(window: &Window, theme: &Theme) -> Vec<FontResolution> {
    let typography = &theme.typography;
    let faces = [
        (typography.family.clone(), FontWeight::NORMAL),
        (typography.family.clone(), typography.medium),
        (typography.mono_family.clone(), FontWeight::NORMAL),
        (typography.mono_family.clone(), typography.medium),
        (MISSING_FONT.into(), FontWeight::NORMAL),
        (MISSING_FONT.into(), typography.medium),
    ];
    faces
        .into_iter()
        .map(|(family, weight)| {
            let mut font = gpui::font(family.clone());
            font.weight = weight;
            FontResolution {
                font_id: window.text_system().resolve_font(&font).0,
                probe_width: shaped_width_in(
                    window,
                    family.clone(),
                    theme,
                    FONT_PROBE,
                    px(14.),
                    weight,
                ),
                family,
                weight: weight.0,
            }
        })
        .collect()
}

fn manifest(fixture: &FixtureWindow, window: &Window, cx: &App) -> Manifest {
    let visuals = settings::visuals(cx);
    // The manifest declares the unperturbed intent; the perturbation is
    // the rendering's fault the comparison must catch.
    let theme = visuals.theme;
    let geometry = &theme.geometry;
    let scenario = fixture.scenario;
    let frame = frame(&theme, scenario.client);

    let replay = replay(scenario, &theme);
    let mut captures = replay.captures;
    let mut steps = replay.steps;
    let mut keycaps = Vec::new();
    let mut tiles = Vec::new();
    match scenario.family {
        Family::Keycap => {
            let mut y = KEYCAP_INSET;
            for group in KEY_GROUPS {
                let record = declared_group(
                    window,
                    &theme,
                    (group.binding, &parse_binding(group.binding)),
                    group.group,
                    group.style,
                    (KEYCAP_INSET, y),
                );
                y += record.rect.height + KEYCAP_GAP;
                keycaps.push(record);
            }
        }
        Family::Tiles => {
            let mut x = KEYCAP_INSET;
            for &(size, tone, _) in TILES {
                let metrics = size.metrics(&theme);
                let side = f32::from(metrics.size);
                tiles.push(TileRecord {
                    size: match size {
                        TileSize::Row => "row",
                        TileSize::Slot => "slot",
                        TileSize::Mini => "mini",
                    },
                    tone: if tone == IconTone::Command {
                        "command"
                    } else {
                        "app"
                    },
                    rect: Rect {
                        x,
                        y: KEYCAP_INSET,
                        width: side,
                        height: side,
                    },
                    glyph: f32::from(metrics.glyph),
                });
                x += side + TILE_GAP;
            }
        }
        Family::Root => {
            // The footer, as `ui::footer` lays it out: the mark and the
            // hint from the left padding; the Actions button at the right
            // padding, the rule, and the primary action before it — each
            // button centered in the strip's first line below the 1px rule.
            let rule = 1.;
            let line = frame.footer.height - rule;
            // Layout places the buttons on whole pixels: 7.5px of room
            // above a 34px button in the 49px line rounds to 8.
            let top =
                (frame.footer.y + rule + (line - f32::from(geometry.action_height)) / 2.).round();
            let cap_top =
                top + (f32::from(geometry.action_height) - f32::from(geometry.keycap_height)) / 2.;
            let keyboard = settings::keyboard_of(cx);
            let invoke = keyboard
                .binding(KeyboardAction::InvokeSelectedAction)
                .clone();
            let open = keyboard.binding(KeyboardAction::OpenActions).clone();
            let button_width = |label: &str, caps: f32| {
                2. * f32::from(geometry.action_padding_x)
                    + shaped_width(
                        window,
                        &theme,
                        label,
                        theme.typography.footer_size,
                        theme.typography.medium,
                    )
                    + f32::from(geometry.action_gap)
                    + caps
            };
            let right =
                frame.footer.x + frame.footer.width - f32::from(geometry.footer_padding_right);
            // The Actions button and its keys, then the rule before it.
            let mut actions_keys = declared_group(
                window,
                &theme,
                ("open-actions", &open),
                Some("footer-actions"),
                CapStyle::Regular,
                (0., cap_top),
            );
            let actions_width = button_width("Actions", actions_keys.rect.width);
            let actions_button = Rect {
                x: right - actions_width,
                y: top,
                width: actions_width,
                height: f32::from(geometry.action_height),
            };
            let shift = right - f32::from(geometry.action_padding_x) - actions_keys.rect.width;
            actions_keys.rect.x += shift;
            for cap in &mut actions_keys.caps {
                cap.rect.x += shift;
            }
            keycaps.push(actions_keys);
            let divider_height = f32::from(geometry.footer_divider_height);
            let divider = Rect {
                x: actions_button.x - f32::from(geometry.footer_buttons_gap) - 1.,
                y: frame.footer.y + rule + (line - divider_height) / 2.,
                width: 1.,
                height: divider_height,
            };
            let primary_right = divider.x - f32::from(geometry.footer_buttons_gap);
            // Laid out at 0 to measure it, then placed.
            let mut primary = declared_group(
                window,
                &theme,
                ("invoke", &invoke),
                Some("footer-primary"),
                CapStyle::Accent,
                (0., cap_top),
            );
            let cap_width = primary.rect.width;
            let mark_size = f32::from(geometry.footer_mark_size);
            let mark = Rect {
                x: frame.footer.x + f32::from(geometry.footer_padding_left),
                y: frame.footer.y + rule + (line - mark_size) / 2.,
                width: mark_size,
                height: mark_size,
            };
            for capture in &mut captures {
                capture.action_button = capture.action.map(|action| {
                    let width = button_width(action, cap_width);
                    Rect {
                        x: primary_right - width,
                        y: top,
                        width,
                        height: f32::from(geometry.action_height),
                    }
                });
                // The hint: its parts from after the mark, 6px apart.
                let parts = footer::hint_parts(
                    capture.actions_open,
                    crate::keyboard::binding_keys(&invoke),
                    crate::keyboard::binding_keys(&open),
                    crate::keyboard::escape_keys(),
                );
                let mut x = mark.x + mark_size + f32::from(geometry.footer_lead_gap);
                // The hint's room: up to the gap before the first button.
                let room = capture.action_button.map_or(divider.x, |button| button.x)
                    - f32::from(geometry.footer_lead_gap);
                let mut hint = Vec::new();
                for part in parts {
                    let (text, keys, width) = match part {
                        footer::HintPart::Text(text) => {
                            let width = shaped_width(
                                window,
                                &theme,
                                &text,
                                theme.typography.footer_size,
                                theme.typography.regular,
                            );
                            (text.to_string(), false, width)
                        }
                        // A sequence by its caps' labels, as the
                        // reference's DOM reads them ("CtrlK").
                        footer::HintPart::Keys(keys) => {
                            let width = keys_width(window, &theme, &keys, CapStyle::Regular);
                            let labels = keys.keys.iter().map(|key| key.cap.as_ref()).collect();
                            (labels, true, width)
                        }
                    };
                    hint.push(DeclaredHintPart {
                        text,
                        keys,
                        rect: Rect {
                            x,
                            y: frame.footer.y + rule,
                            width,
                            height: line,
                        },
                        clipped: x + width > room,
                    });
                    x += width + f32::from(geometry.footer_hint_gap);
                }
                capture.footer = Some(DeclaredFooter {
                    mark,
                    hint,
                    divider,
                    actions_button,
                    actions_pressed: capture.actions_open,
                });
            }
            let shift = primary_right - f32::from(geometry.action_padding_x) - cap_width;
            primary.rect.x += shift;
            for cap in &mut primary.caps {
                cap.rect.x += shift;
            }
            keycaps.push(primary);
            for capture in &mut captures {
                for row in &mut capture.rows {
                    declare_trailing(window, &theme, row);
                }
            }
            // A click on the Actions button lands at its center.
            let (x, y) = actions_button.center();
            for step in &mut steps {
                if step.step
                    == (Step::Click {
                        target: ACTIONS_BUTTON,
                    })
                {
                    step.point = Some((x, y));
                }
            }
        }
    }

    let bounds = window.bounds();
    let viewport = window.viewport_size();
    Manifest {
        kind: "pane-visual-fixture-manifest",
        scenario,
        perturbation: fixture.perturbation.map(Perturbation::name),
        material: match visuals.material.window_appearance() {
            gpui::WindowBackgroundAppearance::Opaque => "opaque",
            _ => "glass",
        },
        appearance: if theme.panel_solid.lightness < 0.5 {
            "dark"
        } else {
            "light"
        },
        window: WindowRecord {
            bounds: [
                f32::from(bounds.origin.x),
                f32::from(bounds.origin.y),
                f32::from(bounds.size.width),
                f32::from(bounds.size.height),
            ],
            viewport: [f32::from(viewport.width), f32::from(viewport.height)],
            scale_factor: window.scale_factor(),
        },
        declared: DeclaredTokens {
            geometry: GeometryRecord {
                search_height: f32::from(geometry.search_height),
                search_padding_x: f32::from(geometry.search_padding_x),
                search_gap: f32::from(geometry.search_gap),
                search_size: f32::from(theme.typography.search_size),
                row_min_height: f32::from(geometry.row_min_height),
                row_radius: f32::from(geometry.row_radius),
                row_padding_x: f32::from(geometry.row_padding_x),
                row_gap: f32::from(geometry.row_gap),
                row_list_gap: f32::from(geometry.row_list_gap),
                row_title_size: f32::from(theme.typography.row_title_size),
                row_subtitle_size: f32::from(theme.typography.row_subtitle_size),
                list_padding_top: f32::from(geometry.list_padding_top),
                list_padding_bottom: f32::from(geometry.list_padding_bottom),
                list_padding_x: f32::from(geometry.list_padding_x),
                tile_size: f32::from(geometry.tile.size),
                tile_radius: f32::from(geometry.tile.radius),
                footer_height: f32::from(geometry.footer_height),
                footer_padding_left: f32::from(geometry.footer_padding_left),
                footer_padding_right: f32::from(geometry.footer_padding_right),
                action_height: f32::from(geometry.action_height),
                action_radius: f32::from(geometry.action_radius),
                action_padding_x: f32::from(geometry.action_padding_x),
                action_gap: f32::from(geometry.action_gap),
                keycap_height: f32::from(geometry.keycap_height),
                keycap_radius: f32::from(geometry.keycap_radius),
                keycap_padding_x: f32::from(geometry.keycap_padding_x),
                keycap_compact_height: f32::from(geometry.keycap_compact_height),
                keycap_compact_padding_x: f32::from(geometry.keycap_compact_padding_x),
                key_gap: f32::from(geometry.key_gap),
            },
            colors: ColorRecord {
                panel_solid: Hex(theme.panel_solid),
                keycap_background: Hex(theme.keycap_background),
                keycap_bottom: Hex(theme.keycap_bottom),
                alias_text: Hex(theme.alias_text),
                keycap_text: Hex(theme.keycap_text),
                accent: Hex(theme.accent),
                accent_ink: Hex(theme.accent_ink),
                panel_top_highlight: Hex(theme.panel_top_highlight),
                row_hover: Hex(theme.row_hover),
                row_selected: Hex(theme.row_selected),
                row_selected_border: Hex(theme.row_selected_border),
                footer_tint: Hex(theme.footer_tint),
                hairline: Hex(theme.hairline),
                hairline_soft: Hex(theme.hairline_soft),
                tile_background: Hex(theme.tile_background),
                tile_foreground: Hex(theme.tile_foreground),
                text_title: Hex(theme.text_title),
                text_muted: Hex(theme.text_muted),
                text_query: Hex(theme.text_query),
                text_placeholder: Hex(theme.text_placeholder),
                popover_solid: Hex(theme.popover_solid),
                popover_edge: Hex(theme.popover_edge),
                footer_mark: Hex(theme.footer_mark),
                footer_button_text: Hex(theme.footer_button_text),
                control_hover: Hex(theme.control_hover),
                footer_button_open: Hex(theme.footer_button_open),
                footer_divider: Hex(theme.footer_divider),
                action_selected: Hex(theme.action_selected),
                action_text: Hex(theme.action_text),
                action_icon: Hex(theme.action_icon),
                action_rule: Hex(theme.action_rule),
                actions_dimmer: Hex(theme.actions_dimmer),
            },
        },
        search_header: frame.search,
        list: frame.list,
        footer: frame.footer,
        captures,
        steps,
        keycaps,
        tiles,
        font_resolution: font_resolution(window, &theme),
        fonts: crate::ui::FONTS
            .iter()
            .map(|&(file, bytes)| FontRecord {
                file,
                bytes: bytes.len(),
                sha256: format!("{:X}", Sha256::digest(bytes)),
            })
            .collect(),
    }
}

/// Runs the fixture application: the production settings (in the
/// isolated data directory, or in memory), the embedded fonts, the
/// production key bindings, and the fixture window at the scenario's
/// client size. Writes the manifest once the window has drawn its first
/// frame, then runs until the window closes.
pub fn run(options: FixtureOptions) -> Result<(), String> {
    let scenario = find_scenario(&options.scenario)?;
    let FixtureOptions {
        data_dir,
        theme,
        material,
        perturbation,
        manifest: manifest_path,
        ..
    } = options;
    // A scenario that names its own appearance renders in it, whatever the
    // run's theme.
    let theme = scenario.theme.map(str::to_owned).or(theme);
    gpui_platform::application().run(move |cx: &mut App| {
        settings::init_with_overrides(
            data_dir.clone(),
            settings::Overrides::parse(theme.as_deref(), material.as_deref()),
            cx,
        );
        if let Err(error) = crate::ui::load_fonts(cx) {
            eprintln!("the fixture's fonts could not be loaded: {error:#}");
        }
        // The production bindings, as the launcher registers them: the
        // text editing keys, then the selection keys in the query field,
        // then the launcher's own (Back among them).
        let keyboard = settings::keyboard_of(cx);
        let text_editing = bind_text_editing(cx);
        root_search::bind_keys(cx, &text_editing, &keyboard);
        actions_panel::bind_keys(cx, &text_editing);
        crate::keyboard::bind_keys(cx, &keyboard);

        let (width, height) = scenario.client;
        let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_background: settings::window_background(cx),
            // Shown without activation: a capture run never takes the
            // foreground from whatever the operator is doing. The capture
            // helper posts its input to this window and reads its client
            // with PrintWindow, neither of which needs the foreground.
            focus: false,
            titlebar: Some(TitlebarOptions {
                title: Some(format!("Pane visual fixture - {}", scenario.name).into()),
                appears_transparent: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        let window = match cx.open_window(window_options, |window, cx| {
            #[cfg(target_os = "windows")]
            crate::prefer_rounded_window_corners(window);
            cx.new(|cx| {
                let fixture = FixtureWindow::new(scenario, perturbation, cx);
                // Root search opens with the query field focused; the
                // fixture does the same, so typing reaches the field the
                // moment the window appears.
                window.focus(&fixture.query.focus_handle(cx), cx);
                fixture
            })
        }) {
            Ok(window) => window,
            Err(error) => {
                eprintln!("the fixture window could not open: {error:#}");
                cx.quit();
                return;
            }
        };
        // The manifest is written after the first frame, from the view's
        // own state and the window's own measurements; the capture helper
        // waits for the file before it captures anything.
        let written = window.update(cx, |_, window, cx| {
            let path = manifest_path.clone();
            window.on_next_frame(move |window, cx| {
                let result = window
                    .root::<FixtureWindow>()
                    .flatten()
                    .ok_or_else(|| "the fixture window has no fixture view".to_owned())
                    .and_then(|fixture| write_json(&path, &manifest(fixture.read(cx), window, cx)));
                if let Err(error) = result {
                    eprintln!("{error}");
                    cx.quit();
                }
            });
            cx.notify();
        });
        if let Err(error) = written {
            eprintln!("the fixture window closed before its first frame: {error:#}");
            cx.quit();
            return;
        }
        let fixture_window = window.window_id();
        cx.on_window_closed(move |cx, closed| {
            if closed == fixture_window {
                cx.quit();
            }
        })
        .detach();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme() -> Theme {
        Theme::dark()
    }

    fn declared_captures(scenario: &Scenario, theme: &Theme) -> Vec<DeclaredCapture> {
        replay(scenario, theme).captures
    }

    fn scenario(name: &str) -> &'static Scenario {
        find_scenario(name).expect("the scenario is registered")
    }

    fn capture<'a>(captures: &'a [DeclaredCapture], name: &str) -> &'a DeclaredCapture {
        captures
            .iter()
            .find(|capture| capture.name == name)
            .unwrap_or_else(|| panic!("no capture {name}"))
    }

    #[test]
    fn every_reference_scenario_renders_at_the_reference_root_client() {
        for scenario in scenarios().iter().filter(|scenario| scenario.reference) {
            assert_eq!(scenario.client, (760., 518.), "{}", scenario.name);
        }
        // The launcher window opens at the same client.
        assert_eq!(ROOT_CLIENT, crate::ui::shell::LAUNCHER_CLIENT);
    }

    #[test]
    fn the_root_frame_divides_the_client_as_the_reference_does() {
        // 64 search + 404 list + 50 footer = 518, edge to edge: the
        // panel's inset ring takes no layout space.
        let frame = frame(&theme(), ROOT_CLIENT);
        assert_eq!((frame.search.x, frame.search.y), (0., 0.));
        assert_eq!(frame.search.width, 760.);
        assert_eq!(frame.search.y + frame.search.height, frame.list.y);
        assert_eq!(frame.list.y + frame.list.height, frame.footer.y);
        assert_eq!(frame.footer.y + frame.footer.height, 518.);
        assert_eq!(frame.list.height, 404.);
    }

    #[test]
    fn light_and_narrow_frames_are_native_only_adaptations() {
        let light = scenario("launcher-frame-light");
        assert_eq!(light.theme, Some("light"));
        assert!(!light.reference && light.frame);
        let narrow = scenario("launcher-frame-narrow");
        assert_eq!(narrow.client, NARROW_CLIENT);
        assert!(!narrow.reference && narrow.frame);
        assert!(scenario("launcher-frame").reference);
    }

    #[test]
    fn the_narrow_list_scrolls_the_last_row_into_view_above_the_footer() {
        let captures = declared_captures(scenario("launcher-frame-narrow"), &theme());
        let rest = capture(&captures, "narrow-rest");
        // At rest the first rows show and the rest are below the fold.
        assert!(rest.rows[0].visible && rest.rows[0].selected);
        assert!(!rest.rows.last().unwrap().visible);
        let last = capture(&captures, "narrow-last-selected");
        let selected = last.rows.last().unwrap();
        assert!(selected.selected && selected.visible);
        // Scrolled by the least that shows it: its bottom on the list's.
        let list_bottom = NARROW_CLIENT.1 - 50.;
        assert_eq!(selected.rect.y + selected.rect.height, list_bottom);
        assert!(!last.rows[0].visible);
    }

    #[test]
    fn pending_scenarios_name_their_tickets_and_cover_the_three_board_sizes() {
        for pending in pending_scenarios() {
            assert!(
                pending
                    .ticket
                    .starts_with("https://github.com/hoangvu12/pane/issues/"),
                "{}",
                pending.name
            );
            assert!(
                find_scenario(pending.name)
                    .unwrap_err()
                    .contains(pending.ticket)
            );
        }
        for client in [ROOT_CLIENT, SETTINGS_CLIENT, CLIPBOARD_CLIENT] {
            assert!(pending_scenarios().iter().any(|p| p.client == client));
        }
        // Store and the snap HUD stay source-only references.
        assert!(
            pending_scenarios()
                .iter()
                .all(|p| p.board != "store" && p.board != "window-manager")
        );
    }

    #[test]
    fn the_root_rows_are_the_reference_boards_suggested_then_commands() {
        let titles: Vec<_> = ROOT_ROWS.iter().map(|row| row.title).collect();
        assert_eq!(
            titles,
            [
                "Figma",
                "Clipboard History",
                "Left Half",
                "Search Files",
                "Plugin Store",
                "Toggle Dark Mode",
                "Lock Screen",
                "Settings"
            ]
        );
    }

    #[test]
    fn rows_are_laid_out_down_the_list_from_its_top_padding() {
        let captures = declared_captures(scenario("root-rest"), &theme());
        let rows = &capture(&captures, "rest").rows;
        assert_eq!(rows.len(), ROOT_ROWS.len());
        for (index, row) in rows.iter().enumerate() {
            // Row i below the header, the list's top padding and the
            // "Commands" label (30 and the list's gap), then one row height
            // and one list gap per row before it.
            let y = 64. + 4. + (30. + 2.) + index as f32 * (44. + 2.);
            assert_eq!(
                row.rect,
                Rect {
                    x: 10.,
                    y,
                    width: 760. - 20.,
                    height: 44.
                }
            );
        }
        // With the label above them the eight rows still show whole: only
        // the list's bottom padding runs past its 404px.
        assert!(rows.iter().all(|row| row.visible));
        let last = rows.last().unwrap().rect;
        assert_eq!(last.y + last.height, 64. + 402.);
        let sections = &capture(&captures, "rest").sections;
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].label, "Commands");
        assert_eq!(sections[0].rect.y, 64. + 4.);
    }

    #[test]
    fn the_selected_scenario_moves_one_row_per_key_and_records_each_step() {
        let captures = declared_captures(scenario("root-selected"), &theme());
        let selected: Vec<_> = captures
            .iter()
            .map(|capture| capture.rows.iter().position(|row| row.selected))
            .collect();
        assert_eq!(selected, [Some(0), Some(1), Some(2)]);
        assert_eq!(captures[2].after.len(), 2);
    }

    #[test]
    fn moving_the_pointer_onto_a_row_selects_it() {
        let captures = declared_captures(scenario("root-hover"), &theme());
        let hover = capture(&captures, "hover");
        let (x, y) = hover.pointer.expect("the pointer is in the window");
        assert!(hover.rows[1].rect.contains((x, y)));
        assert!(hover.rows[1].hovered && hover.rows[1].selected);
        assert!(!hover.rows[0].selected && !hover.rows[0].hovered);
        assert_eq!(hover.action, Some("Run Command"));
        assert!(capture(&captures, "rest").pointer.is_none());

        let captures = declared_captures(scenario("root-selected-hover"), &theme());
        let both = capture(&captures, "selected-hover");
        assert!(both.rows[2].hovered && both.rows[2].selected);
    }

    #[test]
    fn a_resting_pointer_keeps_its_wash_while_the_keys_move_the_selection() {
        let captures = declared_captures(scenario("root-pointer-keys"), &theme());
        let selected = |name: &str| {
            capture(&captures, name)
                .rows
                .iter()
                .position(|row| row.selected)
        };
        assert_eq!(selected("pointed"), Some(1));
        // Down with the pointer resting on row 1: row 2 is selected, row
        // 1 keeps the hover wash.
        let down = capture(&captures, "down-under-pointer");
        assert_eq!(selected("down-under-pointer"), Some(2));
        assert!(down.rows[1].hovered && !down.rows[1].selected);
        // Moving again over row 1, a few pixels on, selects it again.
        assert_eq!(selected("moved-again"), Some(1));
    }

    #[test]
    fn the_footer_action_follows_the_selected_rows_kind() {
        // The reference names the primary action by the selected row's
        // kind: Figma is an application, Clipboard History a command.
        let captures = declared_captures(scenario("root-selected"), &theme());
        let actions: Vec<_> = captures.iter().map(|capture| capture.action).collect();
        assert_eq!(
            actions,
            [
                Some("Open Application"),
                Some("Run Command"),
                Some("Run Command")
            ]
        );
    }

    #[test]
    fn the_actions_scenario_opens_filters_empties_and_closes_the_panel_only() {
        let captures = declared_captures(scenario("root-actions"), &theme());
        let names: Vec<_> = captures.iter().map(|capture| capture.name).collect();
        assert_eq!(names, ["selected", "open", "filtered", "empty", "closed"]);
        let labels = |capture: &DeclaredCapture| -> Vec<String> {
            capture
                .actions
                .as_ref()
                .map(|panel| panel.rows.iter().map(|row| row.label.clone()).collect())
                .unwrap_or_default()
        };
        // Clipboard History has an alias and a hotkey: its configuration
        // entries offer to change them, under the "Pane" label.
        assert_eq!(
            labels(&captures[1]),
            ["Run Command", "Change Hotkey…", "Change Alias…"]
        );
        let open = captures[1].actions.as_ref().expect("the panel is open");
        assert_eq!((open.groups.len(), open.rules.len()), (1, 1));
        assert!(open.rows[0].selected);
        // The filter narrows them, and drops the group's rule and label.
        assert_eq!(labels(&captures[2]), ["Change Alias…"]);
        let filtered = captures[2].actions.as_ref().expect("still open");
        assert_eq!((filtered.groups.len(), filtered.rules.len()), (0, 0));
        // Then to nothing, which the panel says.
        let empty = captures[3].actions.as_ref().expect("still open");
        assert!(empty.rows.is_empty());
        assert_eq!(
            empty.empty.as_ref().map(|note| note.text.as_str()),
            Some(actions_panel::NO_MATCH)
        );
        // Escape closes the panel only: the query and the target stay.
        assert!(captures[4].actions.is_none());
        for capture in &captures {
            assert_eq!(capture.query, "");
            assert_eq!(capture.action, Some("Run Command"));
        }
    }

    #[test]
    fn the_declared_panel_sits_above_the_footer_at_the_right() {
        let theme = theme();
        let frame = frame(&theme, ROOT_CLIENT);
        let captures = declared_captures(scenario("root-actions"), &theme);
        let panel = captures[1].actions.as_ref().expect("the panel is open");
        assert_eq!(panel.rect.x + panel.rect.width, ROOT_CLIENT.0 - 10.);
        assert_eq!(panel.rect.width, 320.);
        assert_eq!(panel.rect.y + panel.rect.height, frame.footer.y - 8.);
        assert_eq!(
            panel.search.y + panel.search.height,
            panel.rect.y + panel.rect.height,
            "the search row closes the panel"
        );
        let header = panel.header.as_ref().expect("the target is named");
        assert_eq!(header.rect.y, panel.rect.y);
        // Header 30, list padding 6: the first entry; then 36 and the 1px
        // gap, the rule's 9 and the gap, the label's 26 and the gap.
        assert_eq!(panel.rows[0].rect.y, panel.rect.y + 36.);
        assert_eq!(panel.rules[0].y, panel.rows[0].rect.y + 36. + 1. + 4.);
        assert_eq!(
            panel.groups[0].rect.y,
            panel.rows[0].rect.y + 36. + 1. + 9. + 1.
        );
        assert_eq!(panel.rows[1].rect.y, panel.groups[0].rect.y + 26. + 1.);
        assert_eq!(panel.dimmer, frame.list);
    }

    #[test]
    fn pointer_steps_resolve_to_the_center_of_their_row() {
        let replay = replay(scenario("root-hover"), &theme());
        let pointer = replay
            .steps
            .iter()
            .find(|step| matches!(step.step, Step::Pointer { .. }))
            .expect("the hover scenario moves the pointer");
        let row = &replay.captures[1].rows[1].rect;
        assert_eq!(pointer.point, Some(row.center()));
        assert!(
            replay
                .steps
                .iter()
                .filter(|step| step.point.is_some())
                .count()
                == 1
        );
    }

    #[test]
    fn typing_filters_by_title_and_escape_returns_to_rest() {
        let captures = declared_captures(scenario("root-focus"), &theme());
        let typed = capture(&captures, "focus-typed");
        assert_eq!(typed.query, "clip");
        let titles: Vec<_> = typed.rows.iter().map(|row| row.title).collect();
        assert_eq!(titles, ["Clipboard History"]);
        assert!(typed.rows[0].selected);
        let back = capture(&captures, "back-to-rest");
        assert_eq!(back.query, "");
        assert_eq!(back.rows.len(), ROOT_ROWS.len());
        assert!(back.rows[0].selected);
    }

    #[test]
    fn rows_after_an_unavailable_reason_declare_only_a_height_floor() {
        let captures = declared_captures(scenario("root-unavailable"), &theme());
        let rows = &capture(&captures, "rest").rows;
        assert!(!rows[0].height_is_floor);
        assert!(rows[1].height_is_floor && rows[1].unavailable.is_some());
    }

    #[test]
    fn the_perturbations_change_exactly_the_value_they_name() {
        let base = theme();
        let mut padded = theme();
        Perturbation::RowPaddingPlus4.apply(&mut padded);
        assert_eq!(
            padded.geometry.list_padding_x,
            base.geometry.list_padding_x + px(4.)
        );
        assert_eq!(hex(padded.row_selected), hex(base.row_selected));

        let mut filled = theme();
        Perturbation::SelectedFill.apply(&mut filled);
        assert_eq!(hex(filled.row_selected), "#FFFFFF40");
        assert_ne!(hex(base.row_selected), "#FFFFFF40");
        assert_eq!(filled.geometry.list_padding_x, base.geometry.list_padding_x);

        let mut hovered = theme();
        Perturbation::HoverFill.apply(&mut hovered);
        assert_eq!(hex(hovered.row_hover), "#FFFFFF33");
        assert_eq!(hex(hovered.row_selected), hex(base.row_selected));
        for &(name, perturbation) in PERTURBATIONS {
            assert_eq!(Perturbation::parse(name), Ok(Some(perturbation)));
            assert_eq!(perturbation.name(), name);
        }
    }

    #[test]
    fn arguments_parse_into_a_run_or_a_registry() {
        let args = |text: &str| text.split(' ').map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(
            parse_args(args(
                "--scenario root-rest --manifest m.json --perturb selected-fill --material opaque"
            )),
            Ok(Command::Run(FixtureOptions {
                scenario: "root-rest".into(),
                data_dir: None,
                theme: None,
                material: Some("opaque".into()),
                perturbation: Some(Perturbation::SelectedFill),
                manifest: "m.json".into(),
            }))
        );
        assert_eq!(
            parse_args(args("--registry r.json")),
            Ok(Command::Registry("r.json".into()))
        );
        assert!(parse_args(args("--scenario root-rest")).is_err());
        assert!(parse_args(args("--perturb sideways")).is_err());
        assert!(parse_args(args("--manifest")).is_err());
    }

    #[test]
    fn the_registry_serializes_steps_for_the_capture_helpers() {
        let json = serde_json::to_value(Registry {
            kind: "pane-visual-fixture-registry",
            scenarios: scenarios(),
            pending: pending_scenarios(),
        })
        .unwrap();
        let hover = &json["scenarios"][1];
        assert_eq!(hover["name"], "root-hover");
        assert_eq!(
            hover["steps"][1],
            serde_json::json!({ "action": "pointer", "row": 1, "nudge": 0 })
        );
        assert_eq!(hover["client"], serde_json::json!([760.0, 518.0]));
    }
}
