use std::process::exit;
use x11rb::connection::Connection;
use x11rb::errors::{ConnectionError, ReplyError, ReplyOrIdError};
use x11rb::protocol::xproto::*; // TODO: Bad practice here
use x11rb::protocol::ErrorKind;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

use log::{debug, error, warn};

struct WindowManager<'a> {
    conn: &'a RustConnection,
    screen: &'a Screen,
    windows: Vec<Window>,
    running: bool,
}

impl<'a> WindowManager<'a> {
    /// Creates and registers new window manager
    ///
    /// TODO: Check if this is using best practices
    pub fn new(conn: &'a RustConnection, screen: &'a Screen) -> WindowManager<'a> {
        let wm = WindowManager {
            conn,
            screen,
            windows: Vec::default(),
            running: true,
        };

        // TODO: Error handing
        wm.register_wm().unwrap();

        wm
    }

    /// Registers self as a window manager
    ///
    /// This is done by registering SUBSTRUCTURE_REDIRECT and SUBSTRUCTURE_NOTIFY on the root
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
                // NOTE: Switch to eprintln later
                // I am using println so that I only need to read stdout for debugging
                println!("Error: Another window manager is already running.");
                exit(1);
            } else {
                response
            }
        } else {
            response
        }
    }

    /// Manages preexisting windows
    pub fn scan_existing(&self) {}

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
            Event::UnmapNotify(event) => self.handle_unmap_notify(event),
            Event::ConfigureRequest(event) => self.handle_configure_request(event)?,
            Event::MapRequest(event) => self.handle_map_request(event)?,
            Event::Expose(event) => {}
            Event::EnterNotify(event) => {}
            Event::ButtonPress(event) => {}
            Event::ButtonRelease(event) => {}
            Event::MotionNotify(event) => {}
            _ => {}
        };

        Ok(())
    }

    fn handle_unmap_notify(&mut self, event: UnmapNotifyEvent) {
        // Remove window from list of windows
        debug!("Got unmap notify from {}", event.window);
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
        self.windows.push(event.window);
        Ok(())
    }

    fn handle_expose() {}

    fn handle_enter_notify() {}

    fn handle_button_press() {}

    fn handle_button_release() {}

    fn handle_motion_notify() {}
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
    let mut wm = WindowManager::new(&conn, screen);
    debug!("Created WM!");

    // Take control of existing windows
    wm.scan_existing();
    debug!("Got existing!");

    // Run main event loop and cleanup on exit
    wm.run();
    debug!("Finished running!")
}
