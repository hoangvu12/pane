//! The binding recognizer (#252) driven by synthetic key-event sequences:
//! the pure state machine the Windows half of the hotkey adapter feeds
//! the key events its low-level keyboard hook sees. It is compiled and
//! run on every system, so the chord logic is checked without a Windows
//! machine; its rules are the ones `RegisterHotKey` keeps for the chords
//! Windows accepts, so a binding behaves the same whichever way it is
//! dispatched.

use pane_core::hotkeys::{Decision, INJECTED_TAG, KeyEvent, Recognizer};

/// The virtual-key codes the tests press, as the hook reports them.
const CONTROL: u32 = 0xA2; // VK_LCONTROL
const ALT: u32 = 0xA4; // VK_LMENU
const SHIFT: u32 = 0xA0; // VK_LSHIFT
const G: u32 = 0x47; // 'G'
const H: u32 = 0x48; // 'H'

/// One key event, as the hook would report a physical key.
fn event(key: u32, pressed: bool) -> KeyEvent {
    KeyEvent {
        key,
        scan: 0,
        pressed,
        injected: false,
        tag: 0,
        time: 0,
    }
}

/// Presses `modifiers` down, then `key` down and up, then the modifiers
/// up, and returns the recognizer's decisions in order.
fn press(recognizer: &mut Recognizer, modifiers: &[u32], key: u32) -> Vec<Decision> {
    let mut events = Vec::new();
    for modifier in modifiers {
        events.push((*modifier, true));
    }
    events.push((key, true));
    events.push((key, false));
    for modifier in modifiers.iter().rev() {
        events.push((*modifier, false));
    }
    events
        .into_iter()
        .map(|(key, pressed)| recognizer.step(event(key, pressed)))
        .collect()
}

/// Ctrl+Alt+G, bound as binding 1.
fn bound() -> Recognizer {
    let mut recognizer = Recognizer::default();
    recognizer.add(1, [true, true, false, false], G);
    recognizer
}

#[test]
fn a_chord_fires_when_its_key_is_pressed_with_its_modifiers_held() {
    let mut recognizer = bound();
    assert_eq!(
        press(&mut recognizer, &[CONTROL, ALT], G),
        vec![
            Decision::Pass,    // Ctrl down
            Decision::Pass,    // Alt down
            Decision::Fire(1), // G down: the chord, reported and swallowed
            Decision::Swallow, // G up: Pane swallowed its press
            Decision::Pass,    // Alt up
            Decision::Pass,    // Ctrl up
        ]
    );
}

#[test]
fn a_chord_needs_exactly_its_modifiers_and_its_key() {
    let mut recognizer = bound();
    // An extra modifier held makes another chord, as the system's own
    // registration treats it.
    for decision in press(&mut recognizer, &[CONTROL, ALT, SHIFT], G) {
        assert_eq!(decision, Decision::Pass);
    }
    // Another key with the modifiers held is not the chord.
    for decision in press(&mut recognizer, &[CONTROL, ALT], H) {
        assert_eq!(decision, Decision::Pass);
    }
}

#[test]
fn keys_pressed_between_the_modifiers_and_the_key_do_not_break_the_chord() {
    let mut recognizer = bound();
    let mut decisions = Vec::new();
    for (key, pressed) in [
        (CONTROL, true),
        (ALT, true),
        (H, true),
        (H, false),
        (G, true),
        (G, false),
        (ALT, false),
        (CONTROL, false),
    ] {
        decisions.push(recognizer.step(event(key, pressed)));
    }
    // The G between the modifiers and the key passed through, and the
    // chord fired anyway, as the system's registration would have.
    assert_eq!(decisions[2], Decision::Pass);
    assert_eq!(decisions[4], Decision::Fire(1));
}

#[test]
fn a_key_pressed_while_already_down_is_its_auto_repeat() {
    let mut recognizer = bound();
    recognizer.step(event(CONTROL, true));
    recognizer.step(event(ALT, true));
    assert_eq!(recognizer.step(event(G, true)), Decision::Fire(1));
    // Holding the key: its repeats fire nothing and repeat nothing, as
    // the registration's no-repeat does.
    assert_eq!(recognizer.step(event(G, true)), Decision::Swallow);
    assert_eq!(recognizer.step(event(G, true)), Decision::Swallow);
    assert_eq!(recognizer.step(event(G, false)), Decision::Swallow);
    // Pressed again once released: the chord fires again.
    assert_eq!(recognizer.step(event(G, true)), Decision::Fire(1));
}

#[test]
fn a_stuck_modifier_does_not_fire_a_phantom_chord_once_resynchronized() {
    let mut recognizer = bound();
    recognizer.step(event(CONTROL, true));
    recognizer.step(event(ALT, true));
    // The releases happened while the secure desktop of a lock or a UAC
    // prompt had the keyboard: Pane missed them. A session unlock
    // re-reads the modifiers' real state.
    recognizer.resync([false, false, false, false]);
    assert_eq!(recognizer.step(event(G, true)), Decision::Pass);
    // The modifiers really held after the re-read: the chord fires.
    recognizer.step(event(CONTROL, true));
    recognizer.step(event(ALT, true));
    assert_eq!(recognizer.step(event(G, true)), Decision::Fire(1));
}

#[test]
fn a_modifier_event_that_contradicts_the_state_asks_for_a_re_read() {
    let mut recognizer = Recognizer::default();
    recognizer.step(event(CONTROL, true));
    // Ctrl is held; a second press of it can only mean Pane missed its
    // release while the secure desktop had the keyboard.
    assert_eq!(recognizer.step(event(CONTROL, true)), Decision::Resync);
    // The caller re-reads the real state, and the keyboard goes on.
    recognizer.resync([true, false, false, false]);
    assert_eq!(recognizer.step(event(CONTROL, false)), Decision::Pass);
    assert_eq!(recognizer.step(event(CONTROL, true)), Decision::Pass);
}

#[test]
fn pane_s_own_injected_keys_are_passed_through_untouched() {
    let mut recognizer = bound();
    // The keys Pane itself injects — the Start-menu mask, a paste, a
    // simulated copy — carry Pane's tag: they are never the user's, and
    // never Pane's own bindings.
    let tagged = |key: u32, pressed: bool| KeyEvent {
        tag: INJECTED_TAG,
        ..event(key, pressed)
    };
    for (key, pressed) in [(CONTROL, true), (ALT, true), (G, true), (G, false)] {
        assert_eq!(recognizer.step(tagged(key, pressed)), Decision::Pass);
    }
    // Untouched: they left no state, so a real chord still fires.
    assert_eq!(
        press(&mut recognizer, &[CONTROL, ALT], G),
        vec![
            Decision::Pass,
            Decision::Pass,
            Decision::Fire(1),
            Decision::Swallow,
            Decision::Pass,
            Decision::Pass,
        ]
    );
    // And they were not the keyboard's, so the watchdog counted none.
    assert_eq!(recognizer.physical_events(), 6);
}

#[test]
fn keys_other_tools_inject_fire_the_binding_but_are_not_the_user_s_to_take() {
    let mut recognizer = bound();
    // A remapper — or the real-input adapter test, or the GUI smoke's
    // SendKeys — injects the chord with a tag of its own, not Pane's: the
    // keys the user pressed reach Pane, and the binding works.
    let injected = |key: u32, pressed: bool| KeyEvent {
        injected: true,
        ..event(key, pressed)
    };
    assert_eq!(recognizer.step(injected(CONTROL, true)), Decision::Pass);
    assert_eq!(recognizer.step(injected(ALT, true)), Decision::Pass);
    assert_eq!(recognizer.step(injected(G, true)), Decision::Injected(1));
    // The keys are the tool's, though, not Pane's to take: they pass
    // through, and their releases too.
    assert_eq!(recognizer.step(injected(G, false)), Decision::Pass);
    // They are not the user's either: the watchdog's evidence counts the
    // keyboard's events alone, so a hook that saw only injected keys
    // looks dead to it.
    assert_eq!(recognizer.physical_events(), 0);
    recognizer.step(event(H, true));
    assert_eq!(recognizer.physical_events(), 1);
}

#[test]
fn an_unbound_chord_no_longer_fires_and_an_empty_recognizer_needs_no_hook() {
    let mut recognizer = bound();
    assert!(!recognizer.is_empty());
    assert_eq!(
        press(&mut recognizer, &[CONTROL, ALT], G)[2],
        Decision::Fire(1)
    );
    recognizer.remove(1);
    assert!(recognizer.is_empty());
    // The chord fires no more.
    for decision in press(&mut recognizer, &[CONTROL, ALT], G) {
        assert_eq!(decision, Decision::Pass);
    }
}
