use crate::{Keybind, WindowManager};
use x11rb::protocol::xproto::ModMask;

pub fn bind_keys<'a>(wm: &mut WindowManager<'a>) {
    let keybinds: Vec<Keybind> = vec![
        Keybind(24, ModMask::M4, WindowManager::quit),
        Keybind(44, ModMask::M4, WindowManager::focus_next),
        Keybind(45, ModMask::M4, WindowManager::focus_prev),
    ];

    wm.set_keybinds(keybinds);
}
