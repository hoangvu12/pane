//! Test fixture: a designed view whose tree and answers are JSON written
//! by hand against Pane's current contract (`wit/extension.wit`), not by
//! pane-extension, so that Pane's reading of them is checked on its own
//! (see `crates/pane-core/tests/designed_views.rs`).
//!
//! Its screen is a counter — a column of a text ("Count: N"), a text
//! naming how many times the view was drawn ("Renders: N", which a
//! refresh raises) and a row of the buttons below, each naming its own
//! callback id — whose presses answer what the next drawing is, once,
//! then the counter again:
//!
//! - "Increment" counts one up, and only when the event's `key` names the
//!   button, so the test checks that Pane sends the node's key;
//! - "Answer an error" makes the next `render` answer an error;
//! - "Answer an over-limit tree" makes it answer a tree of 10,001 nodes;
//! - "Draw an unknown node with a fallback" makes it answer a tree whose
//!   root is a type Pane does not know, with a `fallback` text;
//! - "Draw an unknown node without a fallback" the same without one, with
//!   a text child;
//! - "Draw a newer minor tree" a tree naming version 1.9;
//! - "Draw another major's tree" a tree naming version 2.0;
//! - "Answer an unreadable tree" makes it answer text that is not JSON;
//! - "Answer an unknown callback" answers the event itself with an error.
//!
//! Every render also answers `refresh-after-ms` as the state's
//! `refresh_ms` says, so the tests of #236 drive Pane's refreshing of a
//! view through it — each button below sets it, its presses' answers
//! asking from then on, and the hand-written values ride outside any SDK:
//!
//! - "Answer refresh 10ms" asks for 10 ms, below the 100 ms floor;
//! - "Answer refresh 25h" asks for 25 hours, above the 24 h ceiling;
//! - "Answer refresh 1s" asks for a second;
//! - "Stop refreshing" asks for none.
//!
//! It cannot use `pane-extension`, which writes the tree itself, so it
//! supplies the allocator, panic handler, byte comparisons and
//! `cabi_realloc`.
#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::Cell;
use core::ffi::c_void;

wit_bindgen::generate!({ path: "../../../wit", world: "extension" });

use exports::pane::extension::command::{
    CustomView, FieldValue, FormError, Frame, Guest, GuestCustomView, GuestView, LaunchRecord,
    Outcome, Rendered, UiEvent, View, ViewEvent,
};

/// The version of the UI component set the fixture writes.
const COMPONENT_SET: &str = "1.0";

/// What the next drawing answers.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Next {
    /// The counter, as a version 1.0 tree.
    Counter,
    /// An error, once.
    Error,
    /// A tree of more nodes than the limit, once.
    OverLimit,
    /// A tree whose root is unknown, with a fallback, once.
    UnknownWithFallback,
    /// A tree whose root is unknown, without a fallback, once.
    UnknownWithoutFallback,
    /// A tree naming a newer minor version, once.
    NewerMinor,
    /// A tree naming another major version, once.
    OtherMajor,
    /// Text that is not JSON, once.
    Unreadable,
}

/// The view's state, kept in the resource.
struct State {
    count: Cell<u32>,
    next: Cell<Next>,
    /// How many times the view was drawn, refreshes included.
    renders: Cell<u32>,
    /// What every render answers `refresh-after-ms` as, which the refresh
    /// buttons below set: None until one is pressed.
    refresh_ms: Cell<Option<u32>>,
}

// SAFETY: a component's code runs on one thread.
unsafe impl Sync for State {}

/// The fixture's open view, answering Pane's calls.
struct Designed;

impl GuestView for Designed {
    async fn render(&self, _context: String) -> Result<Rendered, String> {
        let renders = STATE.renders.get();
        STATE.renders.set(renders + 1);
        let next = STATE.next.replace(Next::Counter);
        let tree = match next {
            Next::Counter => counter(),
            Next::Error => return Err("the view failed on purpose".into()),
            Next::OverLimit => over_limit(),
            Next::UnknownWithFallback => format!(
                "{{\"version\":\"{COMPONENT_SET}\",\"root\":{{\"type\":\"marquee\",\
                 \"fallback\":{{\"type\":\"text\",\"text\":\"The fallback\"}}}}}}"
            ),
            Next::UnknownWithoutFallback => format!(
                "{{\"version\":\"{COMPONENT_SET}\",\"root\":{{\"type\":\"video\",\
                 \"children\":[{{\"type\":\"text\",\"text\":\"Inside\"}}]}}}}"
            ),
            Next::NewerMinor => format!(
                "{{\"version\":\"1.9\",\"root\":{{\"type\":\"column\",\
                 \"children\":[{{\"type\":\"text\",\"text\":\"Newer\"}}]}}}}"
            ),
            Next::OtherMajor => format!(
                "{{\"version\":\"2.0\",\"root\":{{\"type\":\"column\",\
                 \"children\":[{{\"type\":\"text\",\"text\":\"Future\"}}]}}}}"
            ),
            Next::Unreadable => {
                return Ok(Rendered {
                    tree: "{\"version\":\"1.0\",\"root\":".into(),
                    refresh_after_ms: None,
                })
            }
        };
        Ok(Rendered {
            tree,
            refresh_after_ms: STATE.refresh_ms.get(),
        })
    }

    async fn handle_event(&self, event: UiEvent) -> Result<Outcome, String> {
        match event.callback {
            1 if event.key == "increment" => {
                STATE.count.set(STATE.count.get() + 1);
            }
            2 => STATE.next.set(Next::Error),
            3 => STATE.next.set(Next::OverLimit),
            4 => STATE.next.set(Next::UnknownWithFallback),
            5 => STATE.next.set(Next::UnknownWithoutFallback),
            6 => STATE.next.set(Next::NewerMinor),
            7 => STATE.next.set(Next::OtherMajor),
            8 => STATE.next.set(Next::Unreadable),
            // The refresh buttons: each makes every render from now on ask
            // Pane to draw the view again as it names (10 ms is below the
            // floor, 25 hours above the ceiling), or not again.
            10 => STATE.refresh_ms.set(Some(10)),
            11 => STATE.refresh_ms.set(Some(25 * 60 * 60 * 1000)),
            12 => STATE.refresh_ms.set(Some(1000)),
            13 => STATE.refresh_ms.set(None),
            _ => return Err(format!("unknown callback: {}", event.callback)),
        }
        Ok(Outcome {
            push: None,
            replace: None,
            pop: None,
        })
    }
}

static STATE: State = State {
    count: Cell::new(0),
    next: Cell::new(Next::Counter),
    renders: Cell::new(0),
    refresh_ms: Cell::new(None),
};

/// The buttons the counter's tree names: (label, key, callback id).
const BUTTONS: [(&str, &str, u32); 13] = [
    ("Increment", "increment", 1),
    ("Answer an error", "error", 2),
    ("Answer an over-limit tree", "over-limit", 3),
    ("Draw an unknown node with a fallback", "with-fallback", 4),
    ("Draw an unknown node without a fallback", "without-fallback", 5),
    ("Draw a newer minor tree", "newer-minor", 6),
    ("Draw another major's tree", "other-major", 7),
    ("Answer an unreadable tree", "unreadable", 8),
    ("Answer an unknown callback", "unknown-callback", 9),
    ("Answer refresh 10ms", "refresh-10", 10),
    ("Answer refresh 25h", "refresh-25h", 11),
    ("Answer refresh 1s", "refresh-1s", 12),
    ("Stop refreshing", "stop-refresh", 13),
];

/// The counter as its tree, with one button per case above and the count
/// of the view's drawings, which a refresh raises.
fn counter() -> String {
    let buttons: Vec<String> = BUTTONS
        .iter()
        .map(|&(label, key, callback)| {
            format!(
                "{{\"type\":\"button\",\"key\":\"{key}\",\"label\":\"{label}\",\
                 \"onPress\":{callback}}}"
            )
        })
        .collect();
    format!(
        "{{\"version\":\"{COMPONENT_SET}\",\"root\":{{\"type\":\"column\",\"gap\":\"m\",\
         \"children\":[{{\"type\":\"text\",\"text\":\"Count: {}\",\"style\":\"title\"}},\
         {{\"type\":\"text\",\"text\":\"Renders: {}\"}},\
         {{\"type\":\"row\",\"gap\":\"s\",\"children\":[{}]}}]}}}}",
        STATE.count.get(),
        STATE.renders.get(),
        buttons.join(","),
    )
}

/// A tree of one node more than the limit, with a count of 10001.
fn over_limit() -> String {
    let mut tree = format!(
        "{{\"version\":\"{COMPONENT_SET}\",\"root\":{{\"type\":\"column\",\"children\":["
    );
    for _ in 0..10_001 {
        tree.push_str("{\"type\":\"text\",\"text\":\"a\"},");
    }
    tree.pop();
    tree.push_str("]}}");
    tree
}

struct Fixture;
export!(Fixture);

/// A view type that is never opened.
enum NoView {}

impl GuestCustomView for NoView {
    async fn render(&self) -> Frame {
        match *self {}
    }

    async fn handle_event(&self, _event: ViewEvent) -> Result<(), String> {
        match *self {}
    }
}

impl Guest for Fixture {
    type CustomView = NoView;
    type View = Designed;

    async fn render(_launch: LaunchRecord) -> Result<String, String> {
        Err("this command opens a designed view".into())
    }

    async fn run(command: String, _launch: LaunchRecord) -> Result<String, String> {
        Err(format!("`{command}` opens a designed view"))
    }

    async fn handle_event(callback: String, _details: String) -> Result<String, String> {
        Err(format!("unknown callback: {callback}"))
    }

    async fn submit_form(item_id: String, _values: Vec<FieldValue>) -> Result<String, FormError> {
        Err(FormError {
            field: None,
            message: format!("unknown form: {item_id}"),
        })
    }

    async fn open_custom_view(item_id: String) -> Result<CustomView, String> {
        Err(format!("unknown view: {item_id}"))
    }

    async fn open_view(_command: String, _launch: LaunchRecord) -> Result<View, String> {
        STATE.count.set(0);
        STATE.next.set(Next::Counter);
        STATE.renders.set(0);
        STATE.refresh_ms.set(None);
        Ok(View::new(Designed))
    }
}

#[global_allocator]
static ALLOC: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    core::arch::wasm32::unreachable()
}

/// Byte comparison, normally supplied by libc; the compiler emits calls to
/// it for string comparisons.
///
/// # Safety
/// `a` and `b` must be valid for reads of `n` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcmp(a: *const c_void, b: *const c_void, n: usize) -> i32 {
    let (a, b) = (a.cast::<u8>(), b.cast::<u8>());
    for i in 0..n {
        // Byte by byte through volatile reads, so that the compiler cannot
        // turn this loop back into a call to memcmp.
        let (x, y) = unsafe { (a.add(i).read_volatile(), b.add(i).read_volatile()) };
        if x != y {
            return i32::from(x) - i32::from(y);
        }
    }
    0
}

/// Equality-only form of [`memcmp`], which the compiler may call instead.
///
/// # Safety
/// `a` and `b` must be valid for reads of `n` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bcmp(a: *const c_void, b: *const c_void, n: usize) -> i32 {
    unsafe { memcmp(a, b, n) }
}

/// Canonical-ABI allocation entry point, normally supplied by `std`.
///
/// # Safety
/// Called only by the component runtime with valid allocation metadata.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cabi_realloc(
    old: *mut u8,
    old_size: usize,
    align: usize,
    new_size: usize,
) -> *mut u8 {
    use alloc::alloc::{Layout, alloc, realloc};
    if new_size == 0 {
        return align as *mut u8;
    }
    let result = if old_size == 0 {
        unsafe { alloc(Layout::from_size_align_unchecked(new_size, align)) }
    } else {
        unsafe {
            realloc(
                old,
                Layout::from_size_align_unchecked(old_size, align),
                new_size,
            )
        }
    };
    if result.is_null() {
        core::arch::wasm32::unreachable();
    }
    result
}
