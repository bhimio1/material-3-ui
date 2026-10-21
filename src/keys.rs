//! Keyboard shortcuts that match their modifiers exactly.

use egui::{Event, Key, Modifiers};

/// Consume a key press with exactly these modifiers. egui's own
/// `consume_shortcut` also matches when Shift or Alt are held extra, so
/// Ctrl+S would swallow Ctrl+Shift+S. Ctrl and Command count as the same
/// key, so `Modifiers::CTRL` and `Modifiers::COMMAND` both work.
///
/// Alt+key and plain character keys also arrive as typed text (Alt+D types
/// "∂" on some layouts); that text is part of the shortcut and is dropped
/// too, so it doesn't land in a focused text field.
pub fn take_key(ctx: &egui::Context, mods: Modifiers, key: Key) -> bool {
    ctx.input_mut(|i| take_key_in(&mut i.events, mods, key))
}

/// [`take_key`] on a plain event list.
pub fn take_key_in(events: &mut Vec<Event>, mods: Modifiers, key: Key) -> bool {
    let pos = events.iter().position(|e| {
        matches!(e, Event::Key { key: k, pressed: true, modifiers: m, .. } if *k == key && same_mods(*m, mods))
    });
    let Some(p) = pos else {
        return false;
    };
    events.remove(p);
    let types_text = mods.alt || key.name().chars().count() == 1;
    if types_text && !ctrl(mods) && matches!(events.get(p), Some(Event::Text(_))) {
        events.remove(p);
    }
    true
}

fn ctrl(m: Modifiers) -> bool {
    m.ctrl || m.command || m.mac_cmd
}

fn same_mods(a: Modifiers, b: Modifiers) -> bool {
    ctrl(a) == ctrl(b) && a.shift == b.shift && a.alt == b.alt
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(key: Key, modifiers: Modifiers) -> Event {
        Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
    }

    #[test]
    fn modifiers_must_match_exactly() {
        let ctrl_shift = Modifiers { ctrl: true, command: true, shift: true, ..Default::default() };
        let mut events = vec![press(Key::S, ctrl_shift)];
        assert!(!take_key_in(&mut events, Modifiers::COMMAND, Key::S), "Ctrl+S doesn't take Ctrl+Shift+S");
        assert!(take_key_in(&mut events, Modifiers::COMMAND | Modifiers::SHIFT, Key::S));
        assert!(events.is_empty());

        // Linux sends ctrl and command together; either spelling matches.
        let ctrl = Modifiers { ctrl: true, command: true, ..Default::default() };
        let mut events = vec![press(Key::S, ctrl), press(Key::S, ctrl)];
        assert!(take_key_in(&mut events, Modifiers::CTRL, Key::S));
        assert!(take_key_in(&mut events, Modifiers::COMMAND, Key::S));
        assert!(!take_key_in(&mut events, Modifiers::COMMAND, Key::S));

        let mut events = vec![press(Key::F, Modifiers::NONE)];
        assert!(!take_key_in(&mut events, Modifiers::COMMAND, Key::F));
        assert!(take_key_in(&mut events, Modifiers::NONE, Key::F));
    }

    #[test]
    fn releases_are_ignored() {
        let mut events = vec![Event::Key {
            key: Key::S,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Modifiers::COMMAND,
        }];
        assert!(!take_key_in(&mut events, Modifiers::COMMAND, Key::S));
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn alt_shortcuts_drop_their_typed_character() {
        let mut events = vec![press(Key::D, Modifiers::ALT), Event::Text("∂".into()), Event::Text("x".into())];
        assert!(take_key_in(&mut events, Modifiers::ALT, Key::D));
        assert_eq!(events, vec![Event::Text("x".into())]);

        let mut events = vec![press(Key::N, Modifiers::NONE), Event::Text("n".into())];
        assert!(take_key_in(&mut events, Modifiers::NONE, Key::N));
        assert!(events.is_empty());

        // Keys that type nothing leave the next typed text alone.
        let mut events = vec![press(Key::Escape, Modifiers::NONE), Event::Text("a".into())];
        assert!(take_key_in(&mut events, Modifiers::NONE, Key::Escape));
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn works_on_a_live_context() {
        let ctx = egui::Context::default();
        let input = egui::RawInput { events: vec![press(Key::N, Modifiers::COMMAND)], ..Default::default() };
        let mut taken = (false, false);
        ctx.run_ui(input, |ui| {
            taken.0 = take_key(ui.ctx(), Modifiers::COMMAND, Key::N);
            taken.1 = take_key(ui.ctx(), Modifiers::COMMAND, Key::N);
        })
        .drop_without_applying_deltas();
        assert_eq!(taken, (true, false), "consumed once");
    }
}
