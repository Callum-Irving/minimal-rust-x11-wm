mod config;

use std::collections::VecDeque;
use std::process::exit;
use x11rb::connection::Connection;
use x11rb::errors::{ConnectionError, ReplyError, ReplyOrIdError};
use x11rb::protocol::xproto::*;
use x11rb::protocol::ErrorKind;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

use log::{debug, error, warn};

pub enum KeybindFunction<'a> {
    FnVoid(fn(&WindowManager<'a>)),
    FnVoidMut(fn(&mut WindowManager<'a>)),
    FnResult(fn(&WindowManager<'a>) -> Result<(), ReplyOrIdError>),
    FnResultMut(fn(&mut WindowManager<'a>) -> Result<(), ReplyOrIdError>),
}

pub struct Keybind<'a>(Keycode, ModMask, KeybindFunction<'a>);

// TODO: Add tags
#[derive(Debug)]
pub struct Client {
    window: Window,
    x: i16,
    y: i16,
    w: u16,
    h: u16,
    border_width: u16,
    // tags: u8 // This is a bit array
    // floating: bool
}

impl Client {
    pub fn new(window: Window, x: i16, y: i16, w: u16, h: u16, border_width: u16) -> Client {
        Client {
            window,
            x,
            y,
            w,
            h,
            border_width,
        }
    }
}

pub struct WindowManager<'a> {
    conn: &'a RustConnection,
    screen: &'a Screen,
    running: bool,
    windows: VecDeque<Client>,
    keybinds: Vec<Keybind<'a>>,
}

impl<'a> WindowManager<'a> {
    /// Creates and registers new window manager
    pub fn new(
        conn: &'a RustConnection,
        screen: &'a Screen,
    ) -> Result<WindowManager<'a>, ReplyError> {
        let wm = WindowManager {
            conn,
            screen,
            running: true,
            windows: VecDeque::new(),
            keybinds: vec![],
        };

        wm.register_wm()?;

        Ok(wm)
    }

    /// Registers self as a window manager
    ///
    /// This is done by registering SUBSTRUCTURE_REDIRECT and SUBSTRUCTURE_NOTIFY on the root.
    fn register_wm(&self) -> Result<(), ReplyError> {
        let change = ChangeWindowAttributesAux::default()
            .event_mask(EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY);

        let response = self
            .conn
            .change_window_attributes(self.screen.root, &change)
            .unwrap()
            .check();

        if let Err(ReplyError::X11Error(ref error)) = response {
            if error.error_kind == ErrorKind::Access {
                error!("Error: Another window manager is already running.");
                exit(1);
            } else {
                response
            }
        } else {
            response
        }
    }

    /// Sets up the window manager
    pub fn setup(&mut self) -> Result<(), ReplyOrIdError> {
        self.scan_existing()?;
        self.grab_keys()?;
        Ok(())
    }

    /// Takes control of preexisting windows
    fn scan_existing(&mut self) -> Result<(), ReplyOrIdError> {
        let tree_reply = self.conn.query_tree(self.screen.root)?.reply()?;

        let mut cookies = Vec::with_capacity(tree_reply.children.len());
        for win in tree_reply.children {
            let attr = self.conn.get_window_attributes(win)?;
            let geom = self.conn.get_geometry(win)?;
            cookies.push((win, attr, geom));
        }
        // Get the replies and manage windows
        for (win, attr, geom) in cookies {
            let (attr, geom) = (attr.reply(), geom.reply());
            if attr.is_err() || geom.is_err() {
                // Just skip this window
                continue;
            }
            let (attr, geom) = (attr.unwrap(), geom.unwrap());

            if !attr.override_redirect && attr.map_state != MapState::UNMAPPED {
                self.manage_window(win, &geom);
            }
        }

        Ok(())
    }

    // TODO: Implement geometry
    fn manage_window(&mut self, window: Window, geom: &GetGeometryReply) {
        self.windows.push_front(Client::new(
            window,
            geom.x,
            geom.y,
            geom.width,
            geom.height,
            geom.border_width,
        ));
    }

    /// Sets up keybinds
    fn grab_keys(&mut self) -> Result<(), ReplyOrIdError> {
        // Ungrab keys to begin
        // modmask any could also be 0000000011111111
        self.conn
            .ungrab_key(Grab::ANY, self.screen.root, ModMask::ANY)?;

        for keybind in self.keybinds.iter() {
            self.conn.grab_key(
                true,
                self.screen.root,
                keybind.1,
                keybind.0,
                GrabMode::ASYNC,
                GrabMode::ASYNC,
            )?;
        }
        self.conn.grab_key(
            true,
            self.screen.root,
            ModMask::ANY,
            Grab::ANY,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
        )?;
        Ok(())
    }

    /// Runs main event loop
    pub fn run(&mut self) {
        while self.running {
            self.conn.flush().unwrap();
            // TODO: handle this error instead of unwrapping
            // the error is when the server closes
            let event = self.conn.wait_for_event().unwrap();
            let mut event_option = Some(event);
            while let Some(event) = event_option {
                // Handle event
                //debug!("Received event: {:?}", event);
                // TODO: Maybe don't unwrap this
                self.handle_event(event).unwrap();
                // Check for new event
                event_option = self.conn.poll_for_event().unwrap();
            }
        }

        self.cleanup();
    }

    fn cleanup(&self) {}

    /// Handle an X11 event
    fn handle_event(&mut self, event: Event) -> Result<(), ReplyOrIdError> {
        match event {
            Event::DestroyNotify(event) => self.handle_destroy_notify(event),
            Event::ConfigureRequest(event) => self.handle_configure_request(event)?,
            Event::MapRequest(event) => self.handle_map_request(event)?,
            Event::MappingNotify(event) => self.handle_mapping_notify(event)?,
            Event::UnmapNotify(event) => self.handle_unmap_notify(event),
            Event::EnterNotify(event) => self.handle_enter_notify(event),
            Event::ButtonPress(event) => self.handle_button_press(event),
            Event::ButtonRelease(event) => self.handle_button_release(event),
            Event::MotionNotify(event) => self.handle_motion_notify(event),
            Event::KeyPress(event) => self.handle_key_press(event)?,
            _ => {}
        };

        Ok(())
    }

    /// Handle destroy notify
    ///
    /// This just checks if the destroyed window was managed and unmanages it if it was.
    fn handle_destroy_notify(&mut self, event: DestroyNotifyEvent) {
        // This removes all windows that are equal to event.window
        self.windows.retain(|x| x.window != event.window);
        // TODO: Refocus just in case the focused window was destroyed
    }

    /// Handle a mapping notify event
    ///
    /// Updates the window manager's knowledge of the keyboard mapping and grabs keys again.
    fn handle_mapping_notify(&mut self, event: MappingNotifyEvent) -> Result<(), ReplyOrIdError> {
        self.conn
            .get_keyboard_mapping(event.first_keycode, event.count)?;
        if event.request == Mapping::KEYBOARD {
            self.grab_keys()?;
        }
        Ok(())
    }

    fn handle_unmap_notify(&mut self, event: UnmapNotifyEvent) {
        // Remove window from list of windows
        debug!("Got unmap notify from {}", event.window);
        let mut index: Option<usize> = None;
        for (i, client) in self.windows.iter().enumerate() {
            if event.window == client.window {
                index = Some(i);
                break;
            }
        }
        if let Some(index) = index {
            self.windows.remove(index);
            debug!("Removed window at index {}", index);
        } else {
            debug!("The unmapped window wasn't managed by this wm");
        }
        debug!("Length of windows: {:?}", self.windows.len());
    }

    fn handle_configure_request(
        &mut self,
        event: ConfigureRequestEvent,
    ) -> Result<(), ConnectionError> {
        debug!("Got configure request from {}", event.window);
        // TODO: Maybe block clients from changing sibling and/or stack
        let configure_request = ConfigureWindowAux::from_configure_request(&event);
        self.conn
            .configure_window(event.window, &configure_request)?;
        Ok(())
    }

    fn handle_map_request(&mut self, event: MapRequestEvent) -> Result<(), ReplyOrIdError> {
        debug!("Got map request from {}", event.window);
        // Check if override_redirect is set
        let attr = self.conn.get_window_attributes(event.window)?.reply();
        if attr.is_err() {
            warn!(
                "Error getting window attributes from window {}, therefore not mapping",
                event.window
            );
            return Ok(());
        }
        if attr.unwrap().override_redirect {
            return Ok(());
        }

        // If window is mapped already, don't map it again
        if self
            .windows
            .iter()
            .find(|&x| x.window == event.window)
            .is_some()
        {
            return Ok(());
        }

        // TODO: Set border using configure request
        self.conn.map_window(event.window)?;

        let geom = &self.conn.get_geometry(event.window)?.reply()?;
        self.manage_window(event.window, geom);

        debug!("Length of windows: {:?}", self.windows.len());
        Ok(())
    }

    fn handle_enter_notify(&self, event: EnterNotifyEvent) {}

    fn handle_button_press(&self, event: ButtonPressEvent) {}

    fn handle_button_release(&self, event: ButtonReleaseEvent) {}

    /// Handle motion notify
    ///
    /// This is used to resize and move windows using the mouse
    fn handle_motion_notify(&self, event: MotionNotifyEvent) {}

    fn handle_key_press(&mut self, event: KeyPressEvent) -> Result<(), ReplyOrIdError> {
        debug!("Keycode: {}. Modmask: {}", event.detail, event.state);

        let mut keybind_pressed = None;

        for keybind in self.keybinds.iter() {
            if event.detail == keybind.0 && (event.state & 0x7f) == u16::from(keybind.1) {
                keybind_pressed = Some(keybind);
                break;
            }
        }

        if let Some(func) = keybind_pressed {
            match func.2 {
                KeybindFunction::FnVoid(func) => func(self),
                KeybindFunction::FnVoidMut(func) => func(self),
                KeybindFunction::FnResult(func) => func(self)?,
                KeybindFunction::FnResultMut(func) => func(self)?,
            };
        }
        Ok(())
    }

    // Keybind functions

    pub fn focus_next(&mut self) {
        if self.windows.len() < 2 {
            return;
        }
        let temp = self.windows.pop_front().unwrap();
        self.windows.push_back(temp);
    }

    pub fn focus_prev(&mut self) {
        if self.windows.len() < 2 {
            return;
        }
        let temp = self.windows.pop_back().unwrap();
        self.windows.push_front(temp);
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub fn kill_focused(&mut self) -> Result<(), ReplyOrIdError> {
        self.conn.kill_client(self.windows[0].window)?.check()?;
        Ok(())
    }

    pub fn set_keybinds(&mut self, keybinds: Vec<Keybind<'a>>) {
        self.keybinds = keybinds;
    }
}

fn main() {
    simple_logger::SimpleLogger::new().init().unwrap();
    debug!("Starting!");

    let (conn, screen_num) = x11rb::connect(None).unwrap();
    debug!("Got connection!");

    let screen = &conn.setup().roots[screen_num];
    debug!("Got screen!");

    // Create and register window manager
    let mut wm = WindowManager::new(&conn, screen).unwrap();
    debug!("Created WM!");

    // Enable keybinds
    config::bind_keys(&mut wm);

    // Take control of existing windows
    wm.setup().unwrap();
    debug!("Got existing!");

    // Run main event loop and cleanup on exit
    wm.run();
    debug!("Finished running!");
}
