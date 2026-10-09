//! Each system's global hotkey adapter against the real system: a shortcut
//! registers, a second registration of it (as another application would
//! make) is refused as taken, and releasing it, or dropping the adapter,
//! frees it for others — on Windows, where a refused registration falls
//! back to Pane's own keyboard hook (#252, ADR 0039), the second
//! registration is taken through the hook instead.
//!
//! On Linux the checks run on a virtual X server of their own (Xvfb, from
//! `PANE_XVFB` or `PATH`), never on the desktop the tests run in, and a key
//! press made there with xdotool (`PANE_XDOTOOL` or `PATH`) is reported.
//! Without Xvfb they are skipped (CI's check job installs it). On
//! Windows the hotkeys are registered with the session the tests run in;
//! the checks that need to inject real keys (`SendInput`) and to reach
//! the keyboard hook behind the adapter's back run only where
//! `PANE_TEST_REAL_INPUT=1` is set (CI's Windows runner sets it), since
//! a runner's session is not a user's desktop. macOS registers hotkeys
//! with the main thread's run loop, which a test thread does not run, so
//! the macOS adapter is checked by the GUI smoke only.

#[cfg(any(target_os = "linux", target_os = "windows"))]
use pane_core::hotkeys::{HotkeyError, Hotkeys, Shortcut, channel};

#[cfg(target_os = "linux")]
mod x11 {
    use super::*;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    use pane_core::hotkeys::X11Hotkeys;

    /// A virtual X server, stopped when dropped.
    struct Xvfb {
        display: String,
        server: Child,
    }

    impl Drop for Xvfb {
        fn drop(&mut self) {
            let _ = self.server.kill();
            let _ = self.server.wait();
        }
    }

    fn tool(variable: &str, name: &str) -> Option<String> {
        if let Some(path) = std::env::var_os(variable) {
            return Some(path.to_string_lossy().into_owned());
        }
        let found = Command::new("sh")
            .args(["-c", &format!("command -v {name}")])
            .output()
            .ok()?;
        found
            .status
            .success()
            .then(|| String::from_utf8_lossy(&found.stdout).trim().to_owned())
    }

    /// Starts Xvfb on a display of its own, or `None` to skip where there is
    /// none.
    fn xvfb() -> Option<Xvfb> {
        let Some(xvfb) = tool("PANE_XVFB", "Xvfb") else {
            eprintln!("skipped: no Xvfb (set PANE_XVFB)");
            return None;
        };
        for _ in 0..20 {
            let number = 200 + (std::process::id() % 500) + fastrand_offset();
            let display = format!(":{number}");
            let server = Command::new(&xvfb)
                .args([&display, "-screen", "0", "640x480x24", "-nolisten", "tcp"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("Xvfb starts");
            let mut xvfb = Xvfb { display, server };
            let deadline = Instant::now() + Duration::from_secs(10);
            while Instant::now() < deadline {
                if let Ok(Some(_)) = xvfb.server.try_wait() {
                    break; // the display was taken; try another
                }
                let socket = format!("/tmp/.X11-unix/X{number}");
                if std::path::Path::new(&socket).exists() {
                    std::thread::sleep(Duration::from_millis(200));
                    return Some(xvfb);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            let _ = xvfb.server.kill();
        }
        panic!("Xvfb did not start");
    }

    fn fastrand_offset() -> u32 {
        use std::time::SystemTime;
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        nanos % 300
    }

    #[test]
    fn a_grabbed_shortcut_is_reported_refused_to_others_and_released() {
        let Some(server) = xvfb() else {
            return;
        };
        let shortcut = Shortcut::parse("ctrl+alt+p").unwrap();
        let (sender, mut presses) = channel();
        let pane = X11Hotkeys::connect(&server.display, sender).expect("connects to Xvfb");
        assert_eq!(pane.unavailable(), None);
        pane.register(&shortcut).expect("registers");

        // Another client grabbing the same keys is refused.
        let (other_sender, _other_presses) = channel();
        let other = X11Hotkeys::connect(&server.display, other_sender).unwrap();
        assert_eq!(other.register(&shortcut), Err(HotkeyError::Taken));
        // A different shortcut is not affected.
        let different = Shortcut::parse("ctrl+alt+o").unwrap();
        other
            .register(&different)
            .expect("another shortcut registers");

        // A real key press on that display, whichever window has focus.
        if let Some(xdotool) = tool("PANE_XDOTOOL", "xdotool") {
            let pressed = Command::new(&xdotool)
                .args(["key", "ctrl+alt+p"])
                .env("DISPLAY", &server.display)
                .env_remove("WAYLAND_DISPLAY")
                .status()
                .expect("xdotool runs");
            assert!(pressed.success());
            let deadline = Instant::now() + Duration::from_secs(5);
            let reported = loop {
                if let Some(press) = presses.try_next() {
                    break Some(press);
                }
                if Instant::now() > deadline {
                    break None;
                }
                std::thread::sleep(Duration::from_millis(20));
            };
            assert_eq!(reported, Some(shortcut.clone()), "the press is reported");
            assert_eq!(presses.try_next(), None, "one press is reported once");
        } else {
            eprintln!("no xdotool: the press is not checked");
        }

        // Released, the shortcut is free for another client.
        pane.unregister(&shortcut);
        other
            .register(&shortcut)
            .expect("the released shortcut registers elsewhere");
    }

    #[test]
    fn dropping_the_adapter_releases_its_grabs() {
        let Some(server) = xvfb() else {
            return;
        };
        let shortcut = Shortcut::parse("ctrl+alt+p").unwrap();
        let (sender, _presses) = channel();
        let pane = X11Hotkeys::connect(&server.display, sender).unwrap();
        pane.register(&shortcut).unwrap();
        // With Caps Lock, Num Lock and Scroll Lock variants too: another
        // client grabbing any of them is refused while Pane holds it.
        let (other_sender, _other_presses) = channel();
        let other = X11Hotkeys::connect(&server.display, other_sender).unwrap();
        assert_eq!(other.register(&shortcut), Err(HotkeyError::Taken));

        drop(pane);
        other
            .register(&shortcut)
            .expect("the dropped adapter's shortcut registers elsewhere");
    }

    #[test]
    fn a_display_that_cannot_be_reached_is_explained() {
        let (sender, _presses) = channel();
        let error = X11Hotkeys::connect(":4999", sender)
            .err()
            .expect("no X server on :4999");
        assert!(!error.is_empty());
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::*;
    use ::windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use ::windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, MOD_ALT,
        MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, RegisterHotKey, SendInput, VIRTUAL_KEY,
    };
    use ::windows::Win32::UI::WindowsAndMessaging::{
        HHOOK, PostMessageW, UnhookWindowsHookEx, WM_INPUT,
    };
    use pane_core::hotkeys::{HookHealth, INJECTED_TAG, Presses, Route, WindowsHotkeys};
    use std::time::{Duration, Instant};

    /// Whether the real-input checks may run here: CI's Windows runner
    /// sets `PANE_TEST_REAL_INPUT` (the setup action), whose session
    /// allows input; without it they pass without looking.
    fn opted_in() -> bool {
        std::env::var("PANE_TEST_REAL_INPUT").is_ok_and(|value| value == "1")
    }

    /// The tag the test's own injected keys carry, distinct from Pane's
    /// (`INJECTED_TAG`), so they are another tool's rather than Pane's
    /// own, which the hook passes through untouched.
    const TEST_TAG: usize = 0x54455354;

    /// The id this test's own registrations use, on this thread.
    const HELD: i32 = 1;

    /// The virtual-key code of the `f11` and `f12` keys, the chords these
    /// checks use.
    fn key_of(shortcut: &Shortcut) -> u32 {
        match shortcut.key() {
            "f11" => 0x7A,
            "f12" => 0x7B,
            other => panic!("no virtual key for {other}"),
        }
    }

    /// Registers `shortcut`'s chord with the session from this test's own
    /// thread, as another application would hold it: the registration
    /// goes when the thread does, which is when the test ends.
    fn held(shortcut: &Shortcut) -> bool {
        let mut modifiers = MOD_NOREPEAT;
        if shortcut.control() {
            modifiers |= MOD_CONTROL;
        }
        if shortcut.alt() {
            modifiers |= MOD_ALT;
        }
        if shortcut.shift() {
            modifiers |= MOD_SHIFT;
        }
        // SAFETY: plain values; with no window the hotkey belongs to this
        // thread, whose end releases it.
        unsafe { RegisterHotKey(None, HELD, modifiers, key_of(shortcut)) }.is_ok()
    }

    /// One key event `SendInput` injects, tagged `tag`.
    fn key_event(key: u32, up: bool, tag: usize) -> INPUT {
        let mut input = INPUT::default();
        input.r#type = INPUT_KEYBOARD;
        input.Anonymous.ki = KEYBDINPUT {
            wVk: VIRTUAL_KEY(key as u16),
            wScan: 0,
            dwFlags: if up {
                KEYEVENTF_KEYUP
            } else {
                KEYBD_EVENT_FLAGS(0)
            },
            time: 0,
            dwExtraInfo: tag,
        };
        input
    }

    /// The virtual-key codes of `shortcut`'s modifiers, as `SendKeys` and
    /// the other tools that inject input send them: the generic codes,
    /// which the hook's recognizer reads as the modifiers they are.
    fn modifiers_of(shortcut: &Shortcut) -> Vec<u32> {
        let mut keys = Vec::new();
        if shortcut.control() {
            keys.push(0x11);
        }
        if shortcut.alt() {
            keys.push(0x12);
        }
        if shortcut.shift() {
            keys.push(0x10);
        }
        if shortcut.super_key() {
            keys.push(0x5B);
        }
        keys
    }

    /// Presses and releases `shortcut`'s chord with `SendInput`, every key
    /// tagged `tag`, as a tool that injects input: the modifiers down, the
    /// key down and up, then the modifiers up.
    fn inject(shortcut: &Shortcut, tag: usize) {
        let key = key_of(shortcut);
        let mut events = Vec::new();
        for modifier in modifiers_of(shortcut) {
            events.push(key_event(modifier, false, tag));
        }
        events.push(key_event(key, false, tag));
        events.push(key_event(key, true, tag));
        for modifier in modifiers_of(shortcut) {
            events.push(key_event(modifier, true, tag));
        }
        // SAFETY: the events are this slice's, all of them.
        let sent = unsafe { SendInput(&events, size_of::<INPUT>() as i32) };
        assert_eq!(sent as usize, events.len(), "SendInput injected the chord");
    }

    /// The next press `presses` reports, within `seconds`; `None` if none
    /// came.
    fn press_within(presses: &mut Presses, seconds: u64) -> Option<Shortcut> {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        loop {
            if let Some(press) = presses.try_next() {
                return Some(press);
            }
            if Instant::now() > deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// The hook's health once a reinstall has been counted, within
    /// `seconds`; `None` if the watchdog did not reinstall in time.
    fn health_within(pane: &WindowsHotkeys, seconds: u64) -> Option<HookHealth> {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        loop {
            if let Some(health) = pane.hook_health()
                && health.reinstalls >= 1
            {
                return Some(health);
            }
            if Instant::now() > deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn a_shortcut_another_application_holds_is_taken_through_the_keyboard_hook() {
        // An unlikely combination, so the session's own shortcuts are not
        // disturbed.
        let shortcut = Shortcut::parse("ctrl+alt+shift+f9").unwrap();
        let (sender, _presses) = channel();
        let pane = WindowsHotkeys::start(sender).expect("starts");
        assert_eq!(pane.unavailable(), None);
        pane.register(&shortcut).expect("registers");
        assert_eq!(pane.route(&shortcut), Route::System);

        // A second Pane, as another application holding the chord: the
        // system refuses the registration to it, and its own keyboard
        // hook takes the binding instead (ADR 0039) — not an error.
        let (other_sender, _other_presses) = channel();
        let other = WindowsHotkeys::start(other_sender).unwrap();
        other
            .register(&shortcut)
            .expect("the hook takes the held chord");
        assert_eq!(other.route(&shortcut), Route::Hook);

        // Released, the chord registers with the system again, as any
        // other application's own registration.
        pane.unregister(&shortcut);
        let (third_sender, _third_presses) = channel();
        let third = WindowsHotkeys::start(third_sender).unwrap();
        third
            .register(&shortcut)
            .expect("the released shortcut registers with the system");
        assert_eq!(third.route(&shortcut), Route::System);
        third.unregister(&shortcut);
        other.unregister(&shortcut);
    }

    #[test]
    fn dropping_the_adapter_releases_its_hotkeys() {
        let shortcut = Shortcut::parse("ctrl+alt+shift+f10").unwrap();
        let (sender, _presses) = channel();
        let pane = WindowsHotkeys::start(sender).unwrap();
        pane.register(&shortcut).unwrap();
        drop(pane);

        let (other_sender, _other_presses) = channel();
        let other = WindowsHotkeys::start(other_sender).unwrap();
        other
            .register(&shortcut)
            .expect("the dropped adapter's shortcut registers elsewhere");
        // With the system, not the hook: the dropped adapter's
        // registration was released.
        assert_eq!(other.route(&shortcut), Route::System);
    }

    #[test]
    fn the_hook_reports_a_chord_injected_with_a_test_tag() {
        if !opted_in() {
            eprintln!("skipped: set PANE_TEST_REAL_INPUT=1 to inject real keys");
            return;
        }
        // An unlikely combination, so the session's own shortcuts are not
        // disturbed. This test's own registration holds it, as another
        // application would, so Windows refuses it to Pane.
        let shortcut = Shortcut::parse("ctrl+alt+shift+f11").unwrap();
        assert!(held(&shortcut), "the test's registration holds the chord");
        // The keys this test injects are another tool's, not Pane's own:
        // Pane's own tagged keys pass through the hook untouched.
        assert_ne!(TEST_TAG, INJECTED_TAG);
        let (sender, mut presses) = channel();
        let pane = WindowsHotkeys::start(sender).expect("starts");

        // No hook while no binding needs one; the first refused
        // registration starts it.
        assert_eq!(pane.hook_handle(), None);
        pane.register(&shortcut)
            .expect("the hook takes the refused chord");
        assert_eq!(pane.route(&shortcut), Route::Hook);
        assert!(pane.hook_handle().is_some(), "the hook is installed");

        // The chord pressed with SendInput, tagged as another tool's (not
        // Pane's own tag, which passes through untouched), is reported.
        inject(&shortcut, TEST_TAG);
        assert_eq!(
            press_within(&mut presses, 5),
            Some(shortcut.clone()),
            "the hook reports the chord"
        );
        assert_eq!(presses.try_next(), None, "one press is reported once");

        // The hook is removed when no binding needs it.
        pane.unregister(&shortcut);
        assert_eq!(pane.hook_handle(), None, "the hook is removed");
        assert_eq!(pane.hook_health(), None);
    }

    #[test]
    fn the_watchdog_repairs_a_hook_removed_behind_the_adapter_s_back() {
        if !opted_in() {
            eprintln!("skipped: set PANE_TEST_REAL_INPUT=1 to inject real keys");
            return;
        }
        let shortcut = Shortcut::parse("ctrl+alt+shift+f12").unwrap();
        assert!(held(&shortcut), "the test's registration holds the chord");
        let (sender, mut presses) = channel();
        let pane = WindowsHotkeys::start(sender).expect("starts");
        pane.register(&shortcut)
            .expect("the hook takes the refused chord");
        assert_eq!(pane.route(&shortcut), Route::Hook);
        let watchdog = pane.watchdog_window().expect("the watchdog's window");
        let hook = pane.hook_handle().expect("the low-level hook");

        // The chord works while the hook is installed.
        inject(&shortcut, TEST_TAG);
        assert_eq!(
            press_within(&mut presses, 5),
            Some(shortcut.clone()),
            "the chord works through the hook"
        );

        // Windows removes the hook behind the adapter's back — a callback
        // that ran past its timeout — and the chord stops working.
        // SAFETY: the handle the adapter reported, unhooked from this
        // thread as Windows itself would remove it.
        unsafe { UnhookWindowsHookEx(HHOOK(hook as *mut _)) }
            .expect("the hook is removed behind the adapter's back");
        inject(&shortcut, TEST_TAG);
        assert_eq!(
            press_within(&mut presses, 2),
            None,
            "the removed hook sees nothing"
        );

        // The watchdog notices: the keyboard's raw key events arrive
        // without the hook seeing any of them. A test cannot produce the
        // keyboard's events — its injections are a tool's, and do not
        // count as the user's — so they are delivered to the watchdog's
        // window as the keyboard would deliver them.
        for _ in 0..8 {
            // SAFETY: plain values, posted to the watchdog's window.
            let _ = unsafe {
                PostMessageW(
                    Some(HWND(watchdog as *mut _)),
                    WM_INPUT,
                    WPARAM(0),
                    LPARAM(0),
                )
            };
        }
        let health = health_within(&pane, 5).expect("the watchdog reinstalled the hook");
        assert!(health.reinstalls >= 1);
        // The chord works again, through the reinstalled hook.
        inject(&shortcut, TEST_TAG);
        assert_eq!(
            press_within(&mut presses, 5),
            Some(shortcut.clone()),
            "the reinstalled hook reports the chord"
        );
    }
}
