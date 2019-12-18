use std::io::{stdin, stdout, Write};
use std::process::{Command, Stdio};
use termion::input::TermRead;
use termion::raw::IntoRawMode;

use gopher;
use gopher::Type;
use menu::MenuView;
use text::TextView;

pub type Key = termion::event::Key;

pub const SCROLL_LINES: usize = 15;
pub const MAX_COLS: usize = 72;

pub struct UI {
    pages: Vec<Box<dyn View>>,
    page: usize,   // currently focused page
    dirty: bool,   // redraw?
    running: bool, // main ui loop running?
}

#[derive(Debug)]
pub enum Action {
    None,              // do nothing
    Back,              // back in history
    Forward,           // also history
    Open(String),      // url
    Redraw,            // redraw everything
    Quit,              // yup
    Clipboard(String), // copy to clipboard
    Unknown,           // handler doesn't know what to do
}

pub trait View {
    fn process_input(&mut self, c: Key) -> Action;
    fn render(&self) -> String;
    fn url(&self) -> String;
    fn set_size(&mut self, cols: usize, rows: usize);
}

impl UI {
    pub fn new() -> UI {
        UI {
            pages: vec![],
            page: 0,
            dirty: true,
            running: true,
        }
    }

    pub fn run(&mut self) {
        while self.running {
            self.draw();
            self.update();
        }
    }

    pub fn draw(&mut self) {
        if self.dirty {
            print!(
                "{}{}{}{}",
                termion::clear::All,
                termion::cursor::Goto(1, 1),
                termion::cursor::Hide,
                self.render()
            );
            self.dirty = false;
        }
    }

    pub fn update(&mut self) {
        if let Action::Quit = self.process_input() {
            self.running = false;
        }
    }

    pub fn render(&mut self) -> String {
        let (cols, rows) = termion::terminal_size().expect("can't get terminal size"); // TODO
        if !self.pages.is_empty() && self.page < self.pages.len() {
            if let Some(page) = self.pages.get_mut(self.page) {
                page.set_size(cols as usize, rows as usize);
                return page.render();
            }
        }
        String::from("N/A")
    }

    pub fn open(&mut self, url: &str) {
        print!("\r\x1b[90mLoading...\x1b[0m\x1b[K");
        stdout().flush();
        self.dirty = true;
        let (typ, host, port, sel) = gopher::parse_url(url);
        let response = gopher::fetch(host, port, sel)
            .map_err(|e| {
                eprintln!("\x1B[91merror loading \x1b[93m{}: \x1B[0m{}[?25h", url, e); // TODO
                std::process::exit(1);
            })
            .unwrap();

        match typ {
            Type::Menu => self.add_page(MenuView::from(url.to_string(), response)),
            Type::Text => self.add_page(TextView::from(url.to_string(), response)),
            Type::HTML => self.add_page(TextView::from(url.to_string(), response)),
            _ => panic!("unknown type: {:?}", typ),
        }
    }

    fn add_page<T: View + 'static>(&mut self, view: T) {
        if !self.pages.is_empty() && self.page < self.pages.len() - 1 {
            self.pages.truncate(self.page + 1);
        }
        self.pages.push(Box::from(view));
        if self.pages.len() > 1 {
            self.page += 1;
        }
    }

    fn process_input(&mut self) -> Action {
        let mut stdout = stdout().into_raw_mode().unwrap();
        stdout.flush().unwrap();

        match self.process_page_input() {
            Action::Redraw => {
                self.dirty = true;
                Action::None
            }
            Action::Open(url) => {
                self.open(&url);
                Action::None
            }
            Action::Back => {
                if self.page > 0 {
                    self.dirty = true;
                    self.page -= 1;
                }
                Action::None
            }
            Action::Forward => {
                if self.page < self.pages.len() - 1 {
                    self.dirty = true;
                    self.page += 1;
                }
                Action::None
            }
            Action::Clipboard(url) => {
                copy_to_clipboard(&url);
                Action::None
            }
            a => a,
        }
    }

    fn process_page_input(&mut self) -> Action {
        let stdin = stdin();
        let page = self.pages.get_mut(self.page).expect("expected Page"); // TODO

        for c in stdin.keys() {
            let key = c.expect("UI error on stdin.keys"); // TODO
            match page.process_input(key) {
                Action::Unknown => match key {
                    Key::Ctrl('q') | Key::Ctrl('c') => return Action::Quit,
                    Key::Left | Key::Backspace => return Action::Back,
                    Key::Right => return Action::Forward,
                    Key::Char('\n') => return Action::Redraw,
                    Key::Ctrl('y') => return Action::Clipboard(page.url()),
                    _ => {}
                },
                action => return action,
            }
        }
        Action::None
    }
}

impl Drop for UI {
    fn drop(&mut self) {
        print!("\x1b[?25h"); // show cursor
    }
}

fn copy_to_clipboard(data: &str) {
    let mut child = spawn_os_clipboard();
    let child_stdin = child.stdin.as_mut().unwrap();
    child_stdin.write_all(data.as_bytes());
}

fn spawn_os_clipboard() -> std::process::Child {
    if cfg!(target_os = "macos") {
        Command::new("pbcopy").stdin(Stdio::piped()).spawn()
    } else {
        Command::new("xclip")
            .args(&["-sel", "clip"])
            .stdin(Stdio::piped())
            .spawn()
    }
    .unwrap() // TODO
}
