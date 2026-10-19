//! Number fields whose drag goes on past the screen edge, like the circle edge in the view.
//!
//! egui's `DragValue` changes by how far the pointer moved each frame, so it stops when the
//! pointer hits the edge of the screen. While a field passed through [`endless`] is dragged,
//! the pointer is hidden and moved back to the middle of the window whenever it nears an
//! edge, and egui is shown where the pointer would be without those jumps.

use egui::{Context, CursorIcon, Event, Id, Pos2, RawInput, Rect, Response, ViewportCommand};

use crate::view::EndlessDrag;

fn key() -> Id {
    Id::new("endless_field_drag")
}

/// Lets this field's drag go on past the screen edge.
pub fn endless(response: Response) -> Response {
    if response.dragged() {
        response.ctx.data_mut(|d| d.insert_temp(key(), true));
    }
    response
}

#[derive(Default)]
pub struct EndlessFields {
    /// The latest real pointer position.
    pointer: Option<Pos2>,
    /// The field drag under way: where the pointer was when it began, and the drag itself.
    drag: Option<(Pos2, EndlessDrag)>,
    /// A jump to ask for at the end of the frame.
    warp: Option<Pos2>,
    /// Whether the pointer has been moved during this drag.
    jumped: bool,
}

impl EndlessFields {
    /// Before each frame: during a field drag, egui sees the pointer without the jumps.
    pub fn input(&mut self, raw: &mut RawInput, window: Rect) {
        let Some((_, drag)) = &mut self.drag else {
            for e in &raw.events {
                match e {
                    Event::PointerMoved(p) | Event::PointerButton { pos: p, .. } => self.pointer = Some(*p),
                    Event::PointerGone => self.pointer = None,
                    _ => {}
                }
            }
            return;
        };
        raw.events.retain_mut(|e| match e {
            Event::PointerMoved(p) | Event::PointerButton { pos: p, .. } => {
                self.pointer = Some(*p);
                if let Some(to) = drag.update(*p, window) {
                    self.warp = Some(to);
                }
                *p = drag.virt;
                true
            }
            // Leaving the window doesn't end the drag, and egui would lose the movement.
            Event::PointerGone => false,
            _ => true,
        });
    }

    /// After each frame: start or end a field drag, keep the pointer hidden and move it.
    pub fn output(&mut self, ctx: &Context) {
        let dragged = ctx.data_mut(|d| d.remove_temp::<bool>(key())).unwrap_or(false);
        match self.drag {
            None if dragged => {
                if let Some(p) = self.pointer {
                    let pressed_at = ctx.input(|i| i.pointer.press_origin()).unwrap_or(p);
                    self.drag = Some((pressed_at, EndlessDrag::new(p)));
                    self.jumped = false;
                }
            }
            Some((start, _)) if !dragged => {
                // Put the pointer back on the field it left; without jumps it is already
                // where the hand moved it.
                if self.jumped {
                    ctx.send_viewport_cmd(ViewportCommand::CursorPosition(start));
                }
                self.drag = None;
                self.warp = None;
            }
            _ => {}
        }
        if self.drag.is_some() {
            ctx.set_cursor_icon(CursorIcon::None);
            if let Some(to) = self.warp.take() {
                ctx.send_viewport_cmd(ViewportCommand::CursorPosition(to));
                self.jumped = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use egui::{DragValue, Modifiers, PointerButton, ViewportId, pos2, vec2};

    use super::*;

    const WINDOW: Rect = Rect { min: pos2(0.0, 0.0), max: pos2(800.0, 600.0) };

    /// Runs one frame with a single endless field; returns its rect and the jumps asked for.
    fn frame(
        ctx: &Context,
        fields: &mut EndlessFields,
        value: &mut f64,
        events: Vec<Event>,
    ) -> (Rect, Vec<ViewportCommand>) {
        let mut raw = RawInput { screen_rect: Some(WINDOW), events, ..Default::default() };
        fields.input(&mut raw, WINDOW);
        let mut rect = Rect::NOTHING;
        let mut out = ctx.run_ui(raw, |ui| {
            // Halfway down, away from the top and bottom edges.
            ui.add_space(WINDOW.height() / 2.0);
            rect = endless(ui.add(DragValue::new(value).speed(1.0))).rect;
            fields.output(ui.ctx());
        });
        // There is no renderer to hand the font texture to.
        out.textures_delta.clear();
        let cmds = out.viewport_output.get(&ViewportId::ROOT).map(|v| v.commands.clone()).unwrap_or_default();
        (rect, cmds)
    }

    fn button(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
    }

    #[test]
    fn field_drag_keeps_going_past_the_window_edge() {
        let ctx = Context::default();
        let mut fields = EndlessFields::default();
        let mut value = 0.0;
        let (field, _) = frame(&ctx, &mut fields, &mut value, vec![]);
        let start = field.center();
        frame(&ctx, &mut fields, &mut value, vec![Event::PointerMoved(start), button(start, true)]);

        // Drag right until the pointer nears the window edge.
        let mut p = start;
        let mut jump = None;
        while jump.is_none() {
            p += vec2(25.0, 0.0);
            assert!(p.x < WINDOW.max.x, "no jump asked for before the edge");
            let (_, cmds) = frame(&ctx, &mut fields, &mut value, vec![Event::PointerMoved(p)]);
            jump = cmds.into_iter().find_map(|c| match c {
                ViewportCommand::CursorPosition(to) => Some(to),
                _ => None,
            });
        }
        let to = jump.unwrap();
        assert_eq!(to, WINDOW.center());
        let before = p.x - start.x;

        // The pointer lands in the middle and moves on another 300 points.
        let mut q = to;
        for _ in 0..6 {
            q += vec2(50.0, 0.0);
            frame(&ctx, &mut fields, &mut value, vec![Event::PointerMoved(q)]);
        }
        let total = before + (q.x - to.x);
        assert!((value - total as f64).abs() <= 2.0, "value {value}, dragged {total}");
        assert!(value as f32 > WINDOW.max.x - start.x, "should pass the window edge, got {value}");

        // Letting go puts the pointer back on the field.
        let (_, cmds) = frame(&ctx, &mut fields, &mut value, vec![button(q, false)]);
        assert!(cmds.contains(&ViewportCommand::CursorPosition(start)), "{cmds:?}");
        assert!(fields.drag.is_none());
    }

    #[test]
    fn short_field_drag_leaves_the_pointer_alone() {
        let ctx = Context::default();
        let mut fields = EndlessFields::default();
        let mut value = 0.0;
        let (field, _) = frame(&ctx, &mut fields, &mut value, vec![]);
        let start = field.center();
        frame(&ctx, &mut fields, &mut value, vec![Event::PointerMoved(start), button(start, true)]);
        let mut cmds = Vec::new();
        for i in 1..=5 {
            let (_, c) =
                frame(&ctx, &mut fields, &mut value, vec![Event::PointerMoved(start + vec2(10.0 * i as f32, 0.0))]);
            cmds.extend(c);
        }
        let (_, c) = frame(&ctx, &mut fields, &mut value, vec![button(start + vec2(50.0, 0.0), false)]);
        cmds.extend(c);
        assert!((value - 50.0).abs() <= 2.0, "value {value}");
        assert!(!cmds.iter().any(|c| matches!(c, ViewportCommand::CursorPosition(_))), "{cmds:?}");
    }
}
