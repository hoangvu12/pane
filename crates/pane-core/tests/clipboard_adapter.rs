//! The Windows clipboard adapter against the real clipboard: it reports
//! each change the test makes, reads the markers password managers set and
//! then withholds the text, names the program owning the clipboard, puts
//! text on it, and reports nothing once its watch is dropped.
//!
//! The test uses only text it puts on the clipboard itself (each starting
//! with a prefix of its own). It saves what was on the clipboard before, in
//! memory only, and puts it back at the end, after the watch is dropped, so
//! the user's clipboard is never reported, kept or written anywhere. Other
//! systems have no adapter yet (their unavailability is checked in
//! `clipboard.rs`).
#![cfg(target_os = "windows")]

use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use pane_core::clipboard::{
    ClipboardSystem, Content, Markers, Observation, Skip, WindowsClipboard, accept, testing,
};

/// How long a change may take to be reported.
const REPORTED: Duration = Duration::from_secs(5);

#[test]
fn the_listener_reports_this_tests_changes_with_their_markers_until_dropped() {
    let saved = testing::save().expect("the clipboard can be saved");
    let checked = std::panic::catch_unwind(check_listener);
    saved
        .restore()
        .expect("the clipboard is put back as it was");
    if let Err(panic) = checked {
        std::panic::resume_unwind(panic);
    }
}

fn check_listener() {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let prefix = format!("pane-clipboard-test-{}-{nanos}-", std::process::id());
    let (sender, reports) = mpsc::channel::<Observation>();
    let ours = prefix.clone();
    let watch = WindowsClipboard
        .watch(Box::new(move |observation| {
            // Only this test's own text, or a withheld change whose text was
            // never read; anything else on the clipboard meanwhile is dropped
            // unseen.
            let keep = match &observation.content {
                Content::Text(text) => text.starts_with(&ours),
                Content::Withheld => true,
                Content::Other => false,
            };
            if keep {
                let _ = sender.send(observation);
            }
        }))
        .expect("Windows can watch the clipboard");
    // The next report with `content`; a change can be reported more than
    // once, so an earlier one reported again is skipped.
    let next = |content: Content| loop {
        let report = reports
            .recv_timeout(REPORTED)
            .unwrap_or_else(|_| panic!("{content:?} is reported"));
        if report.content == content {
            return report;
        }
    };

    // Plain text, owned by a window of this test's process.
    let owner = testing::set_text(&format!("{prefix}plain"), &[]).unwrap();
    let text = format!("{prefix}plain");
    let plain = next(Content::Text(text.clone()));
    drop(owner);
    assert_eq!(accept(&plain, &[]), Ok(text.as_str()));
    assert_eq!(plain.content, Content::Text(text.clone()));
    assert_eq!(plain.markers, Markers::default());
    let program = std::env::current_exe().unwrap();
    let program = program
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_lowercase();
    assert_eq!(
        plain.source.map(|source| source.to_lowercase()),
        Some(program)
    );

    // Each marker a password manager sets withholds the text.
    let marked = [
        ("ExcludeClipboardContentFromMonitorProcessing", 0),
        ("Clipboard Viewer Ignore", 0),
        ("CanIncludeInClipboardHistory", 0),
        ("CanUploadToCloudClipboard", 0),
    ];
    for (format, value) in marked {
        let _owner = testing::set_text(&format!("{prefix}secret"), &[(format, value)]).unwrap();
        let secret = next(Content::Withheld);
        assert_eq!(secret.content, Content::Withheld, "{format}");
        assert!(!secret.markers.allow(), "{format}");
        assert_eq!(accept(&secret, &[]), Err(Skip::Marked));
    }
    let _owner = testing::set_text(
        &format!("{prefix}allowed"),
        &[
            ("CanIncludeInClipboardHistory", 1),
            ("CanUploadToCloudClipboard", 1),
        ],
    )
    .unwrap();
    let allowed = next(Content::Text(format!("{prefix}allowed")));
    assert_eq!(allowed.markers.include_in_history, Some(true));
    assert_eq!(allowed.markers.upload_to_cloud, Some(true));

    // Writing is a change too.
    WindowsClipboard
        .write_text(&format!("{prefix}written ✓"))
        .unwrap();
    next(Content::Text(format!("{prefix}written ✓")));

    // Once the watch is dropped, nothing more is reported.
    drop(watch);
    while reports.try_recv().is_ok() {}
    let _owner = testing::set_text(&format!("{prefix}after"), &[]).unwrap();
    // The sink went with the watch, so the channel is closed and empty.
    assert!(matches!(
        reports.recv_timeout(Duration::from_secs(1)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
}
