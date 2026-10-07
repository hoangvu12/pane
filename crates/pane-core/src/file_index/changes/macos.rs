//! macOS: FSEvents. Pane keeps, per volume holding a root, the volume's
//! FSEvents UUID and the last event id it applied. At start one stream
//! over the roots is opened from that id, so FSEvents replays what changed
//! while Pane was not running, then keeps reporting live changes; the end
//! of the replay is reported as [`Changed::HistoryDone`]. A volume whose
//! UUID changed (its history was reset) has its roots reconciled; a folder
//! FSEvents asks to rescan (its history was purged or coalesced, events
//! were dropped) is reconciled alone; wrapped event ids reconcile every
//! root.
//!
//! The cursor's fields carry FSEvents' values: `volume` the volume's
//! FSEvents UUID, `journal_id` its device number, `next_usn` the event id.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use fsevent_sys as fse;
use fsevent_sys::core_foundation as cf;

use super::{Caught, CaughtUpBy, ChangeSource, Changed, Sink, Watching};
use crate::file_index::journal::JournalCursor;
use crate::file_index::scope::Scope;
use crate::file_index::store::FileIndex;

#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    fn FSEventsCopyUUIDForDevice(dev: libc::dev_t) -> cf::CFRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFUUIDCreateString(alloc: cf::CFAllocatorRef, uuid: cf::CFRef) -> cf::CFStringRef;
}

/// How long FSEvents gathers events before it reports them, in seconds.
const LATENCY: f64 = 0.1;

pub(crate) struct FsEvents;

/// The device holding `root`, and that device's FSEvents UUID.
fn volume_of(root: &Path) -> Option<(libc::dev_t, String)> {
    let dev = std::fs::metadata(root).ok()?.dev() as libc::dev_t;
    // SAFETY: a device number; the UUID and its string are released here.
    unsafe {
        let uuid = FSEventsCopyUUIDForDevice(dev);
        if uuid.is_null() {
            return None;
        }
        let text = CFUUIDCreateString(cf::kCFAllocatorDefault, uuid);
        cf::CFRelease(uuid);
        if text.is_null() {
            return None;
        }
        let mut buffer = [0 as c_char; 64];
        let ok = cf::CFStringGetCString(
            text,
            buffer.as_mut_ptr(),
            buffer.len() as cf::CFIndex,
            cf::kCFStringEncodingUTF8,
        );
        cf::CFRelease(text);
        ok.then(|| {
            CStr::from_ptr(buffer.as_ptr())
                .to_string_lossy()
                .into_owned()
        })
        .map(|uuid| (dev, uuid))
    }
}

/// The cursors as of event `id`, one per volume holding a root.
fn cursors_at(roots: &[PathBuf], id: u64) -> Vec<JournalCursor> {
    let mut cursors: Vec<JournalCursor> = Vec::new();
    for root in roots {
        if let Some((dev, uuid)) = volume_of(root)
            && !cursors.iter().any(|cursor| cursor.volume == uuid)
        {
            cursors.push(JournalCursor {
                volume: uuid,
                journal_id: dev as u64,
                next_usn: i64::try_from(id).unwrap_or(i64::MAX),
            });
        }
    }
    cursors
}

impl ChangeSource for FsEvents {
    fn cursors(&self, scope: &Scope) -> Vec<JournalCursor> {
        // SAFETY: no arguments.
        let id = unsafe { fse::FSEventsGetCurrentEventId() };
        cursors_at(&scope.rules().roots, id)
    }

    fn catch_up(
        &self,
        _index: &FileIndex,
        scope: &Scope,
        cursors: &[JournalCursor],
        _cancel: &AtomicBool,
    ) -> Caught {
        // The replay itself comes through the live stream (`watch`), from
        // the saved ids; here only the volumes are checked.
        let mut reconcile = Vec::new();
        let mut kept = Vec::new();
        let mut note = None;
        for root in &scope.rules().roots {
            match volume_of(root) {
                Some((_, uuid)) => match cursors.iter().find(|cursor| cursor.volume == uuid) {
                    Some(cursor) => {
                        if !kept.contains(cursor) {
                            kept.push(cursor.clone());
                        }
                    }
                    None => {
                        note.get_or_insert_with(|| {
                            format!(
                                "the event history of the volume holding {} is new",
                                root.display()
                            )
                        });
                        reconcile.push(root.clone());
                    }
                },
                // Missing (an unplugged drive): its entries are kept.
                None => {}
            }
        }
        if !reconcile.is_empty() && reconcile.len() == scope.rules().roots.len() {
            return Caught::Reconcile(note.unwrap_or_default());
        }
        Caught::Changes {
            changes: Vec::new(),
            walk: Vec::new(),
            reconcile,
            cursors: kept,
            how: CaughtUpBy::EventHistory,
            note,
        }
    }

    fn watch(
        &self,
        scope: &Scope,
        cursors: &[JournalCursor],
        _folders: Vec<PathBuf>,
        sink: Sink,
    ) -> Result<Box<dyn Watching>, String> {
        let roots = scope.rules().roots.clone();
        let since = cursors
            .iter()
            .filter_map(|cursor| u64::try_from(cursor.next_usn).ok())
            .min()
            .unwrap_or(fse::kFSEventStreamEventIdSinceNow);
        let replaying = since != fse::kFSEventStreamEventIdSinceNow;
        let run_loop: Arc<Mutex<Option<usize>>> = Arc::new(Mutex::new(None));
        let (started, ready) = std::sync::mpsc::channel::<Result<(), String>>();
        let shared_loop = run_loop.clone();
        std::thread::Builder::new()
            .name("pane-file-watch".into())
            .spawn(move || {
                crate::file_index::lower_current_thread();
                run_stream(roots, since, replaying, sink, shared_loop, started);
            })
            .map_err(|error| format!("Pane could not watch for changes: {error}"))?;
        ready
            .recv()
            .map_err(|_| "Pane could not watch for changes".to_owned())??;
        Ok(Box::new(Stream { run_loop }))
    }
}

/// What the stream's callback is given.
struct Info {
    roots: Vec<PathBuf>,
    sink: Sink,
    /// Whether the replayed history is still arriving.
    replaying: bool,
}

/// Creates the stream over `roots` from event `since`, runs it on this
/// thread's run loop until the watch is dropped, then releases it.
fn run_stream(
    roots: Vec<PathBuf>,
    since: u64,
    replaying: bool,
    sink: Sink,
    run_loop: Arc<Mutex<Option<usize>>>,
    started: std::sync::mpsc::Sender<Result<(), String>>,
) {
    let info = Box::into_raw(Box::new(Info {
        roots: roots.clone(),
        sink,
        replaying,
    }));
    // SAFETY: CoreFoundation and CoreServices calls on this thread; every
    // object created is released below, and `info` outlives the stream.
    unsafe {
        let paths =
            cf::CFArrayCreateMutable(cf::kCFAllocatorDefault, 0, &cf::kCFTypeArrayCallBacks);
        for root in &roots {
            let Ok(text) = CString::new(root.as_os_str().as_bytes()) else {
                continue;
            };
            let string = cf::CFStringCreateWithCString(
                cf::kCFAllocatorDefault,
                text.as_ptr(),
                cf::kCFStringEncodingUTF8,
            );
            if !string.is_null() {
                cf::CFArrayAppendValue(paths, string);
                cf::CFRelease(string);
            }
        }
        let context = fse::FSEventStreamContext {
            version: 0,
            info: info.cast::<c_void>(),
            retain: None,
            release: None,
            copy_description: None,
        };
        let stream = fse::FSEventStreamCreate(
            cf::kCFAllocatorDefault,
            callback,
            &context,
            paths,
            since,
            LATENCY,
            fse::kFSEventStreamCreateFlagFileEvents
                | fse::kFSEventStreamCreateFlagNoDefer
                | fse::kFSEventStreamCreateFlagWatchRoot,
        );
        cf::CFRelease(paths);
        if stream.is_null() {
            drop(Box::from_raw(info));
            let _ = started.send(Err("FSEvents refused to watch the folders".into()));
            return;
        }
        let current = cf::CFRunLoopGetCurrent();
        fse::FSEventStreamScheduleWithRunLoop(stream, current, cf::kCFRunLoopDefaultMode);
        if fse::FSEventStreamStart(stream) == 0 {
            fse::FSEventStreamInvalidate(stream);
            fse::FSEventStreamRelease(stream);
            drop(Box::from_raw(info));
            let _ = started.send(Err("FSEvents could not start watching".into()));
            return;
        }
        *run_loop
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(current as usize);
        let _ = started.send(Ok(()));
        cf::CFRunLoopRun();
        fse::FSEventStreamStop(stream);
        fse::FSEventStreamInvalidate(stream);
        fse::FSEventStreamRelease(stream);
        drop(Box::from_raw(info));
    }
}

extern "C" fn callback(
    _stream: fse::FSEventStreamRef,
    info: *mut c_void,
    count: usize,
    paths: *mut c_void,
    flags: *const fse::FSEventStreamEventFlags,
    ids: *const fse::FSEventStreamEventId,
) {
    // SAFETY: FSEvents passes `count` C strings, flags and ids, and the
    // `info` given to the stream, used by this thread only.
    let (info, paths, flags, ids) = unsafe {
        (
            &mut *info.cast::<Info>(),
            std::slice::from_raw_parts(paths.cast::<*const c_char>(), count),
            std::slice::from_raw_parts(flags, count),
            std::slice::from_raw_parts(ids, count),
        )
    };
    let mut changed = Vec::new();
    let mut last = 0;
    for ((&path, &flag), &id) in paths.iter().zip(flags).zip(ids) {
        last = last.max(id);
        // SAFETY: a NUL-terminated path FSEvents gave.
        let path = PathBuf::from(std::ffi::OsStr::from_bytes(
            unsafe { CStr::from_ptr(path) }.to_bytes(),
        ));
        if flag & fse::kFSEventStreamEventFlagHistoryDone != 0 {
            info.replaying = false;
            if !changed.is_empty() {
                let _ = info.sink.send(Changed::Paths(std::mem::take(&mut changed)));
            }
            let _ = info
                .sink
                .send(Changed::HistoryDone(cursors_at(&info.roots, id)));
            continue;
        }
        if flag & fse::kFSEventStreamEventFlagEventIdsWrapped != 0 {
            for root in &info.roots {
                let _ = info.sink.send(Changed::Rescan(root.clone()));
            }
            continue;
        }
        let rescan = fse::kFSEventStreamEventFlagMustScanSubDirs
            | fse::kFSEventStreamEventFlagUserDropped
            | fse::kFSEventStreamEventFlagKernelDropped
            | fse::kFSEventStreamEventFlagRootChanged;
        if flag & rescan != 0 {
            let _ = info.sink.send(Changed::Rescan(path));
            continue;
        }
        changed.push(path);
    }
    if !changed.is_empty() {
        let _ = info.sink.send(Changed::Paths(changed));
    }
    if last != 0 && !info.replaying {
        let _ = info
            .sink
            .send(Changed::Cursors(cursors_at(&info.roots, last)));
    }
}

struct Stream {
    run_loop: Arc<Mutex<Option<usize>>>,
}

impl Watching for Stream {}

impl Drop for Stream {
    fn drop(&mut self) {
        if let Some(run_loop) = *self
            .run_loop
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
        {
            // SAFETY: the run loop of the stream's thread, which runs until
            // stopped here; stopping it from another thread is allowed.
            unsafe { cf::CFRunLoopStop(run_loop as cf::CFRunLoopRef) };
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::channel;
    use std::time::{Duration, Instant};

    use super::*;
    use crate::file_index::ScopeRules;
    use crate::file_index::indexer::Message;

    #[test]
    fn fsevents_replays_what_changed_since_a_saved_event_id_then_reports_live_changes() {
        let dir = tempfile::tempdir().unwrap();
        // FSEvents reports canonical paths (/private/var/… for /var/…).
        let home = std::fs::canonicalize(dir.path()).unwrap().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let scope = Scope::new(ScopeRules::for_home(home.clone()));
        // Pane's cursor, saved, and then a change made while it is not
        // watching.
        let cursors = FsEvents.cursors(&scope);
        assert_eq!(cursors.len(), 1, "one volume");
        let missed = home.join("written while stopped.txt");
        std::fs::write(&missed, "x").unwrap();
        std::thread::sleep(Duration::from_secs(2));

        let (sender, received) = channel();
        let watch = FsEvents
            .watch(&scope, &cursors, Vec::new(), Sink::new(sender, None))
            .unwrap();
        let mut replayed = false;
        let mut done = false;
        let deadline = Instant::now() + Duration::from_secs(20);
        while !(replayed && done) {
            let left = deadline
                .checked_duration_since(Instant::now())
                .expect("FSEvents replayed the change and said it was done");
            match received.recv_timeout(left) {
                Ok(Message::Changed(Changed::Paths(paths))) => {
                    replayed |= paths.contains(&missed);
                }
                Ok(Message::Changed(Changed::HistoryDone(cursors))) => {
                    assert_eq!(cursors.len(), 1);
                    done = true;
                }
                Ok(_) => {}
                Err(error) => panic!("{error}"),
            }
        }
        // Then live changes, on the same stream.
        let live = home.join("live.txt");
        std::fs::write(&live, "x").unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let left = deadline
                .checked_duration_since(Instant::now())
                .expect("FSEvents reported the live change");
            if let Ok(Message::Changed(Changed::Paths(paths))) = received.recv_timeout(left)
                && paths.contains(&live)
            {
                break;
            }
        }
        drop(watch);
    }
}
