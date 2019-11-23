#![allow(unused_must_use)]

extern crate termion;

use std::collections::HashMap;
use std::io::{stdin, stdout, Read, Write};
use std::net::TcpStream;

use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;

#[derive(Debug)]
struct App {
    pages: HashMap<String, Page>,
    cursor: String,
}

#[derive(Debug)]
struct Page {
    body: String,
    cursor: usize,
    url: String,
    links: Vec<Link>,
}

#[derive(Debug)]
struct Link {
    name: String,
    host: String,
    port: String,
    selector: String,
}

#[derive(Debug)]
enum Action {
    None,
    Up,
    Down,
    Open,
    Quit,
}

fn main() {
    let mut app = App {
        pages: HashMap::new(),
        cursor: String::new(),
    };
    app.load("phkt.io", "70", "/");
    loop {
        app.render();
        app.respond();
    }
}

impl App {
    fn load(&mut self, host: &str, port: &str, selector: &str) {
        let mut page = self.fetch(host, port, selector);
        page.parse_links();
        self.pages.insert(page.url.to_string(), page);
        self.cursor = format!("{}:{}{}", host, port, selector);
    }

    fn render(&self) {
        if let Some(page) = self.pages.get(&self.cursor) {
            print!("\x1B[2J\x1B[H{}", page.draw());
        } else {
            println!("{}", "<render error>");
        }
    }

    fn fetch(&self, host: &str, port: &str, selector: &str) -> Page {
        let mut body = String::new();
        TcpStream::connect(format!("{}:{}", host, port))
            .and_then(|mut stream| {
                stream.write(format!("{}\r\n", selector).as_ref());
                Ok(stream)
            })
            .and_then(|mut stream| {
                stream.read_to_string(&mut body);
                Ok(())
            })
            .map_err(|err| {
                eprintln!("err: {}", err);
            });
        Page {
            body: body,
            cursor: 0,
            url: format!("{}:{}{}", host, port, selector),
            links: Vec::new(),
        }
    }

    fn respond(&mut self) {
        match self.pages.get_mut(&self.cursor) {
            None => return,
            Some(page) => match term_input() {
                Action::Up => page.cursor_up(),
                Action::Down => page.cursor_down(),
                Action::Open => {
                    if page.cursor > 0 && page.cursor - 1 < page.links.len() {
                        println!("OPEN: {:?}", page.links[page.cursor - 1]);
                        std::process::exit(0);
                    }
                }
                Action::Quit => return,
                _ => {}
            },
        }
    }
}

impl Page {
    fn cursor_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }
    fn cursor_down(&mut self) {
        if self.cursor < self.links.len() {
            self.cursor += 1;
        }
    }

    fn parse_links(&mut self) {
        if self.links.len() > 0 {
            self.links.clear();
        }
        let mut start = true;
        let mut is_link = false;
        let mut link = (0, 0);
        for (i, c) in self.body.chars().enumerate() {
            if start {
                match c {
                    '0' | '1' => {
                        is_link = true;
                        link.0 = i + 1;
                    }
                    '\n' => continue,
                    _ => is_link = false,
                }
                start = false;
            } else if c == '\n' {
                start = true;
                if is_link && i > link.0 {
                    link.1 = i;
                    let mut line = Vec::new();
                    for s in self.body[link.0..link.1].split('\t') {
                        line.push(s);
                    }
                    self.links.push(Link {
                        name: line[0].to_string(),
                        selector: line[1].to_string(),
                        host: line[2].to_string(),
                        port: line[3].trim_end_matches('\r').to_string(),
                    });
                    is_link = false;
                }
            }
        }
    }

    fn draw(&self) -> String {
        let mut start = true;
        let mut skip_to_end = false;
        let mut links = 0;
        let mut out = String::with_capacity(self.body.len() * 2);
        let mut prefix = "";
        for (i, c) in self.body.chars().enumerate() {
            let mut is_link = false;
            if start {
                match c {
                    'i' => {
                        prefix = "\x1B[93m";
                        is_link = false;
                    }
                    'h' => {
                        prefix = "\x1B[96m";
                        links += 1;
                        is_link = true;
                    }
                    '0' => {
                        prefix = "\x1B[94m";
                        links += 1;
                        is_link = true;
                    }
                    '1' => {
                        prefix = "\x1B[94m";
                        links += 1;
                        is_link = true;
                    }
                    '.' => {
                        if self.body.len() > i + 2
                            && self.body[i..].chars().next().unwrap() == '\r'
                            && self.body[i + 1..].chars().next().unwrap() == '\n'
                        {
                            continue;
                        }
                    }
                    '\r' => continue,
                    '\n' => continue,
                    _ => prefix = "",
                }
                if is_link && self.cursor > 0 && self.cursor == links {
                    out.push_str("\x1b[92;1m*\x1b[0m");
                } else {
                    out.push(' ');
                }
                out.push_str(" ");
                if is_link {
                    out.push_str("\x1B[95m");
                    if links < 10 {
                        out.push(' ');
                    }
                    out.push_str(&links.to_string());
                    out.push_str(". \x1B[0m");
                } else {
                    out.push(' ');
                    out.push_str("\x1B[0m");
                    out.push_str("   ");
                }
                out.push_str(prefix);
                start = false
            } else if skip_to_end {
                if c == '\n' {
                    out.push_str("\r\n\x1B[0m");
                    start = true;
                    skip_to_end = false;
                }
            } else if c == '\t' {
                skip_to_end = true;
            } else {
                out.push(c);
                if c == '\n' {
                    start = true;
                }
            }
        }
        out
    }
}

fn term_input() -> Action {
    let stdin = stdin();
    let mut stdout = stdout().into_raw_mode().unwrap();
    let mut y = 1;
    let mut input = String::new();
    if let Ok((_col, row)) = termion::terminal_size() {
        y = row + 1;
    } else {
        panic!("can't determine terminal size.");
    }

    print!("{}\x1B[92;1m>> \x1B[0m", termion::cursor::Goto(1, y));
    stdout.flush().unwrap();

    for c in stdin.keys() {
        write!(
            stdout,
            "{}{}",
            termion::cursor::Goto(1, y),
            termion::clear::CurrentLine
        )
        .unwrap();
        print!("\x1B[92;1m>> \x1B[0m");

        match c.unwrap() {
            Key::Ctrl('c') | Key::Char('q') => return Action::Quit,
            Key::Char('\n') => return Action::Open,
            Key::Char(c) => input.push(c),
            Key::Alt(c) => print!("Alt-{}", c),
            Key::Up | Key::Ctrl('p') => return Action::Up,
            Key::Down | Key::Ctrl('n') => return Action::Down,
            Key::Ctrl(c) => print!("Ctrl-{}", c),
            Key::Left => print!("<left>"),
            Key::Right => print!("<right>"),
            Key::Backspace | Key::Delete => {
                input.pop();
            }
            _ => print!("Other"),
        }

        print!("{}", input);
        stdout.flush().unwrap();
    }
    Action::None
}
