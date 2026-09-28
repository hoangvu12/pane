//! The custom view screen: a view an extension draws and whose input it
//! handles.
//!
//! Pane paints the view's latest frame (rectangles and text, in order) in a
//! fixed-size area, and keeps focus, the focus ring and the accessibility
//! node itself: one focusable node with the view's role, label and value.
//! While the view has focus, arrow keys, Home and End go to the extension;
//! Tab, Enter and Escape stay with the launcher. Pressing the primary button
//! over the view, moving while it is held (anywhere in the window) and
//! releasing it go to the extension as pointer events, in view coordinates.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, Context, DispatchPhase, FocusHandle, KeyBinding, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Role, Window, actions, canvas, div,
    prelude::*, px, rgb,
};
use pane_core::{CustomViewRole, CustomViewSnapshot, Key, Point, Shape, ViewEvent};

use crate::LauncherWindow;

actions!(custom_view, [Left, Right, Up, Down, Home, End]);

const CONTEXT: &str = "CustomView";

/// Registers the keys a focused custom view receives.
pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("left", Left, Some(CONTEXT)),
        KeyBinding::new("right", Right, Some(CONTEXT)),
        KeyBinding::new("up", Up, Some(CONTEXT)),
        KeyBinding::new("down", Down, Some(CONTEXT)),
        KeyBinding::new("home", Home, Some(CONTEXT)),
        KeyBinding::new("end", End, Some(CONTEXT)),
    ]);
}

/// What the window keeps for the open custom view.
pub(crate) struct CustomViewControls {
    focus: FocusHandle,
    /// Where the view's drawing area was last laid out, to turn window
    /// positions into view coordinates.
    bounds: Rc<Cell<Bounds<Pixels>>>,
}

impl LauncherWindow {
    /// Creates or drops the custom view's controls to match the launcher's
    /// screen: a newly opened view takes focus, and focus returns to the list
    /// when the view closes.
    pub(crate) fn sync_custom_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let open = self.launcher.view().custom_view.is_some();
        match (open, self.custom_view.is_some()) {
            (true, false) => {
                let focus = cx.focus_handle().tab_stop(true);
                window.focus(&focus, cx);
                self.custom_view = Some(CustomViewControls {
                    focus,
                    bounds: Rc::default(),
                });
            }
            (false, true) => {
                self.custom_view = None;
                window.focus(&self.focus_handle, cx);
            }
            (true, true) | (false, false) => {}
        }
    }

    /// Sends `event` to the view and redraws when its answer arrives.
    fn send_view_event(&mut self, event: ViewEvent, window: &mut Window, cx: &mut Context<Self>) {
        let pending = self.launcher.send_view_event(event);
        self.show_until_done(pending, window, cx);
    }

    /// `position`, a window position, in the view's coordinates.
    fn view_point(&self, position: gpui::Point<Pixels>) -> Option<Point> {
        let origin = self.custom_view.as_ref()?.bounds.get().origin;
        let local = position - origin;
        Some(Point {
            x: local.x.as_f32().floor() as i32,
            y: local.y.as_f32().floor() as i32,
        })
    }

    /// The custom view screen for the launcher's snapshot of the view.
    pub(crate) fn render_custom_view(
        &self,
        view: CustomViewSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(controls) = &self.custom_view else {
            return div().into_any_element();
        };
        let frame = view.frame;
        let bounds = controls.bounds.clone();
        let focus = controls.focus.clone();
        let entity = cx.entity().downgrade();
        let (moved, released) = (entity.clone(), entity);
        let drawing = div()
            .debug_selector(|| "custom-view".into())
            .relative()
            .w(px(frame.width as f32))
            .h(px(frame.height as f32))
            .overflow_hidden()
            .children(frame.shapes.into_iter().map(shape))
            .child(
                // Records where the drawing area is, for pointer positions.
                canvas(move |area, _, _| bounds.set(area), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            );
        let view = div()
            .id("custom-view")
            .key_context(CONTEXT)
            .track_focus(&controls.focus)
            .role(match view.role {
                CustomViewRole::ColorWell => Role::ColorWell,
            })
            .aria_label(view.label)
            .aria_value(frame.value)
            .on_action(key_listener::<Left>(cx, Key::Left))
            .on_action(key_listener::<Right>(cx, Key::Right))
            .on_action(key_listener::<Up>(cx, Key::Up))
            .on_action(key_listener::<Down>(cx, Key::Down))
            .on_action(key_listener::<Home>(cx, Key::Home))
            .on_action(key_listener::<End>(cx, Key::End))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    window.focus(&focus, cx);
                    if let Some(at) = this.view_point(event.position) {
                        this.send_view_event(ViewEvent::PointerDown(at), window, cx);
                    }
                }),
            )
            // A drag continues outside the view; the launcher sends moves
            // and the release only while a press over the view is held.
            .on_mouse_move_all(move |event: &MouseMoveEvent, phase, _, window, cx| {
                if phase == DispatchPhase::Bubble && event.pressed_button == Some(MouseButton::Left)
                {
                    moved
                        .update(cx, |this, cx| {
                            if let Some(at) = this.view_point(event.position) {
                                this.send_view_event(ViewEvent::PointerMove(at), window, cx);
                            }
                        })
                        .ok();
                }
            })
            .on_mouse_up_all(move |event: &MouseUpEvent, phase, _, window, cx| {
                if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                    released
                        .update(cx, |this, cx| {
                            if let Some(at) = this.view_point(event.position) {
                                this.send_view_event(ViewEvent::PointerUp(at), window, cx);
                            }
                        })
                        .ok();
                }
            })
            .p_1()
            .rounded_md()
            .border_2()
            .border_color(rgb(0x20252d))
            .focus(|node| node.border_color(rgb(0x8ab4f8)))
            .child(drawing);
        // Fills the body, like the list and the form, so the status line
        // stays at the bottom.
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_start()
            .child(view)
            .into_any_element()
    }
}

/// A listener for the key action `A` that sends `key` to the view.
fn key_listener<A: gpui::Action>(
    cx: &mut Context<LauncherWindow>,
    key: Key,
) -> impl Fn(&A, &mut Window, &mut App) + 'static {
    cx.listener(move |this, _: &A, window, cx| {
        this.send_view_event(ViewEvent::Key(key), window, cx)
    })
}

/// A shape of the view's frame, positioned in the drawing area.
fn shape(shape: Shape) -> AnyElement {
    match shape {
        Shape::Rect {
            x,
            y,
            width,
            height,
            fill,
        } => div()
            .absolute()
            .left(px(x as f32))
            .top(px(y as f32))
            .w(px(width as f32))
            .h(px(height as f32))
            .bg(rgb(fill))
            .into_any_element(),
        Shape::Text {
            x,
            y,
            content,
            color,
        } => div()
            .absolute()
            .left(px(x as f32))
            .top(px(y as f32))
            .whitespace_nowrap()
            .text_color(rgb(color))
            .child(content)
            .into_any_element(),
    }
}
