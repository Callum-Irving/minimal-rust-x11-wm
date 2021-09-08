use crate::{Keybind, KeybindFunction, WindowManager};
use x11rb::protocol::xproto::ModMask;

pub fn bind_keys<'a>(wm: &mut WindowManager<'a>) {
    let keybinds: Vec<Keybind> = vec![
        Keybind(
            24,
            ModMask::M4,
            KeybindFunction::FnVoidMut(WindowManager::quit),
        ),
        Keybind(
            44,
            ModMask::M4,
            KeybindFunction::FnVoidMut(WindowManager::focus_next),
        ),
        Keybind(
            45,
            ModMask::M4,
            KeybindFunction::FnVoidMut(WindowManager::focus_prev),
        ),
        Keybind(
            24,
            ModMask::M4 | ModMask::SHIFT,
            KeybindFunction::FnResultMut(WindowManager::kill_focused),
        ),
    ];

    wm.set_keybinds(keybinds);
}
