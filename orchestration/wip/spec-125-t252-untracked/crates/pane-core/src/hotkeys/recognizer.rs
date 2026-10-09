//! The binding recognizer: a pure state machine that turns key events
//! into chord matches, as Pane's own Windows keyboard hook drives it
//! (`windows_hook`). It is compiled on every system and calls nothing
//! the system provides: the hook feeds it, and the tests drive it with
//! synthetic key-event sequences, so what it decides is checked the same
//! everywhere. Chords only in this slice; the tap kinds, double taps and
//! sides (#260) extend it.
//!
//! A chord — modifier keys held while one other key is pressed — matches
//! when the key goes down while exactly those modifiers are held, and no
//! other key was pressed between the modifiers' last change and the key:
//! the user pressed the whole chord as one gesture. The sequence starts
//! over after a match, so Win+D then Win+E, the modifiers held
//! throughout, are two gestures and both match; a key that completes no
//! chord breaks the sequence until the modifiers change again, as a
//! system registration cannot tell. Repeats of a key already down never
//! match — holding the keys is one press, as `RegisterHotKey`'s
//! `MOD_NOREPEAT` reports one.
//!
//! Events Pane injected itself — tagged with Pane's
//! [`INJECTED_TAG`](super::INJECTED_TAG) — are ignored: Pane's own keys
//! are never taken for the user's, so an injection cannot come back as a
//! press. Keys another tool injected are matched like the user's, as the
//! system's registration matches them, and as the hook's own tests press
//! a chord with.

/// The Ctrl key's modifier bit: one per kind, whichever side of the
/// keyboard is held, as a registered hotkey's modifiers are one per
/// kind.
pub const CTRL: u8 = 1 << 0;
/// The Alt (Option) key's bit.
pub const ALT: u8 = 1 << 1;
/// The Shift key's bit.
pub const SHIFT: u8 = 1 << 2;
/// The Windows key's bit (Command on macOS, Super on Linux).
pub const SUPER: u8 = 1 << 3;

/// The modifiers' virtual-key codes and the bit each sets, in Windows'
/// numbering — the recognizer is pure, so it names the codes itself
/// rather than reaching for the system's constants, which exist only on
/// Windows. Both the left and right sides, and the generic codes an
/// injected event names.
pub const MODIFIER_CODES: [(u32, u8); 11] = [
    (0x10, SHIFT), // VK_SHIFT
    (0x11, CTRL),  // VK_CONTROL
    (0x12, ALT),   // VK_MENU
    (0xA0, SHIFT), // VK_LSHIFT
    (0xA1, SHIFT), // VK_RSHIFT
    (0xA2, CTRL),  // VK_LCONTROL
    (0xA3, CTRL),  // VK_RCONTROL
    (0xA4, ALT),   // VK_LMENU
    (0xA5, ALT),   // VK_RMENU
    (0x5B, SUPER), // VK_LWIN
    (0x5C, SUPER), // VK_RWIN
];

/// One key event as the recognizer takes it: which key, down or up, when
/// it happened, and whether — and by whom — it was injected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    /// The key, as a virtual-key code (Windows' numbering, which the
    /// recognizer's chords are registered in).
    pub code: u32,
    /// Whether the key went down.
    pub down: bool,
    /// Who injected the event, if anyone.
    pub injected: Injected,
    /// When the event happened, in milliseconds of the caller's clock.
    /// A chord does not read it; the tap kinds to come (#260) do.
    pub time: u64,
}

/// Who injected a key event, if anyone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Injected {
    /// The user pressed the key.
    No,
    /// Pane injected it, carrying Pane's tag
    /// ([`INJECTED_TAG`](super::INJECTED_TAG)).
    Pane,
    /// Another tool injected it, carrying a tag that is not Pane's.
    Other,
}

/// One chord the recognizer watches for, as the Windows hook registers
/// it from a [`Shortcut`](super::Shortcut): the modifiers held while one
/// other key is pressed, and the id its watcher knows the chord by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chord {
    /// The modifiers held, as bits ([`CTRL`] and its kin).
    pub modifiers: u8,
    /// The key pressed, as a virtual-key code.
    pub code: u32,
    /// What the watcher calls this chord; a match reports it.
    pub id: u32,
}

/// What feeding one event came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing matched; the tracked state stands.
    None,
    /// The chord with this id matched: its key went down with exactly
    /// its modifiers held, unbroken by another key.
    Matched(u32),
    /// The event contradicts the state tracked: a modifier went down
    /// that is tracked as down (its release was missed, or it is a
    /// repeat), or up that is tracked as up (its press was missed). The
    /// caller re-reads the modifiers the system says are held and
    /// [`Recognizer::resync`] with them, so a missed release never
    /// leaves a modifier stuck down.
    Stale,
}

/// The state machine: the chords watched and the keys as they stand.
/// Beyond the chords — registered off the hot path — it holds only
/// fixed-size state, so feeding an event allocates nothing: the hook's
/// callback can call it without taking time Windows would notice.
#[derive(Debug, Default)]
pub struct Recognizer {
    /// The chords watched.
    chords: Vec<Chord>,
    /// The modifiers tracked as held, as bits.
    held: u8,
    /// Whether a key was pressed since the modifiers last changed: the
    /// chord sequence is broken until they change again.
    broken: bool,
    /// The keys tracked as down, by virtual-key code: 256 bits.
    down: [u64; 4],
}

impl Recognizer {
    /// Watches `chord`, replacing any chord with the same keys.
    pub fn watch(&mut self, chord: Chord) {
        self.chords
            .retain(|watched| (watched.modifiers, watched.code) != (chord.modifiers, chord.code));
        self.chords.push(chord);
    }

    /// Stops watching the chord with `id`; whether there was one.
    pub fn forget(&mut self, id: u32) -> bool {
        let before = self.chords.len();
        self.chords.retain(|watched| watched.id != id);
        self.chords.len() != before
    }

    /// Whether any chord is watched: whether a hook is needed at all.
    pub fn watched(&self) -> bool {
        !self.chords.is_empty()
    }

    /// Takes `event` in, matching a chord when the event completes one.
    pub fn feed(&mut self, event: KeyEvent) -> Outcome {
        if event.injected == Injected::Pane {
            // Pane's own injection: never the user's press, never a
            // change in the user's state.
            return Outcome::None;
        }
        if let Some(bit) = modifier_bit(event.code) {
            let held = self.held & bit != 0;
            if held != event.down {
                // The modifier went the way the state says it should:
                // the sequence starts over.
                if event.down {
                    self.held |= bit;
                } else {
                    self.held &= !bit;
                }
                self.broken = false;
                return Outcome::None;
            }
            // A repeat of a held modifier, or a release whose press was
            // missed — the recognizer cannot tell which, so it asks for
            // the state the system holds.
            return Outcome::Stale;
        }
        let bit = 1u64 << (event.code % 64);
        let word = &mut self.down[(event.code / 64) as usize];
        if !event.down {
            // A release: whether its press was seen or not, the key is
            // up now.
            *word &= !bit;
            return Outcome::None;
        }
        if *word & bit != 0 {
            // A repeat of a held key: one press, not one per repeat.
            return Outcome::None;
        }
        *word |= bit;
        let matched = if self.broken {
            None
        } else {
            self.chords
                .iter()
                .find(|chord| chord.code == event.code && chord.modifiers == self.held)
                .map(|chord| chord.id)
        };
        if matched.is_none() {
            // A key that completes no chord breaks the sequence, until
            // the modifiers change again. A matched chord ends its
            // gesture whole instead, so another chord on the same held
            // modifiers — Win+D then Win+E — matches too.
            self.broken = true;
        }
        match matched {
            Some(id) => Outcome::Matched(id),
            None => Outcome::None,
        }
    }

    /// Re-reads the modifiers from the system: `held` is what it reports
    /// down now. A state that differs from what was tracked follows it,
    /// and the chord sequence starts over — what stood between cannot be
    /// known; one that matches was a repeat, and the sequence stands.
    pub fn resync(&mut self, held: u8) {
        if held != self.held {
            self.held = held;
            self.broken = true;
        }
    }
}

/// The modifier bit `code` sets, if it names a modifier.
fn modifier_bit(code: u32) -> Option<u8> {
    MODIFIER_CODES
        .iter()
        .find(|(modifier, _)| *modifier == code)
        .map(|(_, bit)| *bit)
}
