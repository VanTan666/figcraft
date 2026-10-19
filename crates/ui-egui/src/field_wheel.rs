//! Opt-in wheel stepping. A focused field registers its hit area for the next frame; the frame
//! opens the undo interaction before any field dispatches its ordinary Set/relative command.
use egui::{Context, Event, Id, Modifiers, MouseWheelUnit, Rect, Ui};
use figcraft_engine::Session;

const ID: &str = "number_field_wheel";
const TARGET: &str = "number_field_wheel_target";
const IDLE: f64 = 0.4;

#[derive(Clone)]
struct Target {
    field: Id,
    editor: Id,
    rect: Rect,
}
#[derive(Clone)]
struct Gesture {
    target: Target,
    modifiers: Modifiers,
    last: f64,
    remainder: f64,
    applied_frame: Option<u64>,
}

fn delta(ctx: &Context) -> f64 {
    ctx.input(|i| {
        i.events
            .iter()
            .filter_map(|e| match e {
                Event::MouseWheel { unit, delta, modifiers, .. }
                    if !modifiers.ctrl && !modifiers.command && delta.y.is_finite() && delta.y.abs() >= delta.x.abs() =>
                {
                    Some(f64::from(delta.y) / if *unit == MouseWheelUnit::Point { 40.0 } else { 1.0 })
                }
                _ => None,
            })
            .sum()
    })
}

fn eligible(ctx: &Context, t: &Target) -> bool {
    ctx.memory(|m| m.has_focus(t.field) || m.has_focus(t.editor))
        && ctx.input(|i| i.focused && i.pointer.hover_pos().is_some_and(|p| t.rect.contains(p)))
}

/// End on idle, pointer leave, focus loss or any other edit. This runs even when a field was
/// hidden, so a closed panel never leaves an undo gesture pending.
pub(crate) fn prepare(ctx: &Context, session: &mut Session) {
    let enabled = session.prefs.number_field_wheel;
    let state = ctx.data(|d| d.get_temp::<Gesture>(Id::new(ID)));
    let (now, modifiers, other_edit) = ctx.input(|i| {
        (
            i.time,
            i.modifiers,
            i.events.iter().any(|e| matches!(e, Event::Key { pressed: true, .. } | Event::Text(_) | Event::PointerButton { pressed: true, .. })),
        )
    });
    if state.as_ref().is_some_and(|g| !enabled || now - g.last >= IDLE || modifiers != g.modifiers || !eligible(ctx, &g.target) || other_edit) {
        ctx.data_mut(|d| d.remove::<Gesture>(Id::new(ID)));
        let _ = session.commit_interaction();
    }
    let target = ctx.data_mut(|d| {
        let target = d.get_temp::<Target>(Id::new(TARGET));
        d.remove::<Target>(Id::new(TARGET));
        target
    });
    let by = delta(ctx);
    if enabled
        && !other_edit
        && by.is_finite()
        && by != 0.0
        && let Some(target) = target.filter(|t| eligible(ctx, t))
    {
        let old = ctx.data(|d| d.get_temp::<Gesture>(Id::new(ID)));
        // Do not join a canvas drag, text preview or any other interaction.
        if old.is_none() && session.ds().is_ok_and(|d| d.interaction.is_some()) {
            return;
        }
        if session.begin_interaction("Scroll number").is_err() {
            return;
        }
        let mut state =
            old.filter(|g| g.target.field == target.field).unwrap_or(Gesture { target, modifiers, last: now, remainder: 0.0, applied_frame: None });
        state.last = now;
        ctx.data_mut(|d| d.insert_temp(Id::new(ID), state));
    }
    // An earlier scheduled repaint can win over the latest wheel event's timeout. Re-arm the
    // remainder each frame, otherwise a quiet field could leave its interaction open forever.
    if let Some(g) = ctx.data(|d| d.get_temp::<Gesture>(Id::new(ID))) {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64((IDLE - (now - g.last)).max(0.001)));
    }
}

/// Use raw OS-adjusted deltas, not animated scrolling. Trackpad fractions accumulate to whole
/// steps; one line is a step, 40 points are a step. A horizontal/zoom gesture passes through.
pub(crate) fn steps(ui: &Ui, field: Id, editor: Id, rect: Rect) -> Option<f64> {
    let target = Target { field, editor, rect };
    if !ui.is_enabled() || !eligible(ui.ctx(), &target) || !ui.rect_contains_pointer(rect) {
        return None;
    }
    ui.data_mut(|d| d.insert_temp(Id::new(TARGET), target));
    let mut state = ui.data(|d| d.get_temp::<Gesture>(Id::new(ID))).filter(|g| g.target.field == field)?;
    // Suppress egui's smoothed tail too, even on frames without a raw wheel event.
    ui.input_mut(|i| i.smooth_scroll_delta.y = 0.0);
    let frame = ui.ctx().cumulative_frame_nr();
    if state.applied_frame == Some(frame) {
        return None;
    }
    let by = delta(ui.ctx());
    if !by.is_finite() || by == 0.0 {
        return None;
    }
    state.applied_frame = Some(frame);
    state.remainder = (state.remainder + by).clamp(-1000.0, 1000.0);
    let whole = state.remainder.trunc();
    state.remainder -= whole;
    ui.data_mut(|d| d.insert_temp(Id::new(ID), state));
    (whole != 0.0).then_some(whole)
}
