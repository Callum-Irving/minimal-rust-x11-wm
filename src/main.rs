mod config;

use std::collections::VecDeque;
use std::process::exit;
use x11rb::connection::Connection;
use x11rb::errors::{ConnectionError, ReplyError, ReplyOrIdError};
use x11rb::protocol::xproto::*; // TODO: Bad practice here
use x11rb::protocol::ErrorKind;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

use log::{debug, error, warn};

pub struct Keybind<'a>(Keycode, ModMask, fn(&mut WindowManager<'a>));

pub struct WindowManager<'a> {
    conn: &'a RustConnection,
    screen: &'a Screen,
    running: bool,
    windows: VecDeque<Window>,
    keybinds: Vec<Keybind<'a>>,
}

impl<'a> WindowManager<'a> {
    /// Creates and registers new window manager
    ///
    /// TODO: Check if this is using best practices
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
        self.scan_existing();
        self.grab_keys()?;
        Ok(())
    }

    /// Takes control of preexisting windows
    fn scan_existing(&self) {}

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
            debug!("Grabbed key {} with mod {:?}", keybind.0, keybind.1);
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
            Event::CreateNotify(event) => {}
            Event::DestroyNotify(event) => {}
            Event::ConfigureRequest(event) => self.handle_configure_request(event)?,
            Event::PropertyNotify(event) => {}
            Event::MapRequest(event) => self.handle_map_request(event)?,
            Event::MappingNotify(event) => self.handle_mapping_notify(event)?,
            Event::UnmapNotify(event) => self.handle_unmap_notify(event),
            Event::EnterNotify(event) => {}
            Event::ButtonPress(event) => {}
            Event::ButtonRelease(event) => {}
            Event::MotionNotify(event) => {}
            Event::KeyPress(event) => self.handle_key_press(event),
            _ => {}
        };

        Ok(())
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
        for (i, window) in self.windows.iter().enumerate() {
            if event.window == *window {
                index = Some(i);
                break;
            }
        }
        if let Some(index) = index {
            self.windows.remove(index);
            debug!("Removed window at index {}", index);
        } else {
            // The window wasn't in our vector to begin with
            debug!("The unmapped window wasn't managed by this wm");
        }
        debug!("Windows: {:?}", self.windows);
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
        // TODO: Set border using configure request
        self.conn.map_window(event.window)?;

        self.windows.push_front(event.window);

        debug!("Windows: {:?}", self.windows);
        Ok(())
    }

    fn handle_expose() {}

    fn handle_enter_notify() {}

    fn handle_button_press() {}

    fn handle_button_release() {}

    fn handle_motion_notify() {}

    fn handle_key_press(&mut self, event: KeyPressEvent) {
        debug!("Keycode: {}. Modmask: {}", event.detail, event.state);
        debug!("Mod4: {:?}", u16::from(ModMask::M4));

        let mut keybind_pressed = None;

        for keybind in self.keybinds.iter() {
            // TODO: Clean up event.detail by removing mouse bits
            // TODO: Add way to handle function arguments
            if event.detail == keybind.0 {
                if (event.state & 0x7f) == u16::from(keybind.1) {
                    debug!("Got shortcut");
                    keybind_pressed = Some(keybind);
                    break;
                }
            }
        }

        if let Some(func) = keybind_pressed {
            debug!("Calling func");
            func.2(self);
        }
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

    pub fn set_keybinds(&mut self, keybinds: Vec<Keybind<'a>>) {
        self.keybinds = keybinds;
    }
}

fn main() {
    simple_logger::SimpleLogger::new().init().unwrap();
    debug!("Starting!");

    let (conn, screen_num) = x11rb::connect(None).unwrap();
    debug!("Got connection!");

    // NOTE: Research borrowing (the & symbol here)
    let screen = &conn.setup().roots[screen_num];
    debug!("Got screen!");

    // Create and register window manager
    let mut wm = WindowManager::new(&conn, screen).unwrap();
    debug!("Created WM!");

    config::bind_keys(&mut wm);

    // Take control of existing windows
    wm.setup().unwrap();
    debug!("Got existing!");

    // Run main event loop and cleanup on exit
    wm.run();
    debug!("Finished running!")
}
