use gopher;
use gopher::Type;
use std::io::stdout;
use std::io::Write;
use ui;
use ui::{Action, Key, View, MAX_COLS, SCROLL_LINES};

pub struct Menu {
    pub url: String,          // gopher url
    pub lines: Vec<Line>,     // lines
    pub links: Vec<usize>,    // links (index of line in lines vec)
    pub longest: usize,       // size of the longest line
    pub raw: String,          // raw response
    pub input: String,        // user's inputted value
    pub link: usize,          // selected link
    pub scroll: usize,        // scrolling offset
    pub size: (usize, usize), // cols, rows
    pub wide: bool,           // in wide mode?
}

pub struct Line {
    pub name: String,
    pub url: String,
    pub typ: Type,
    pub link: usize, // link #, if any
}

// direction of a given link relative to the visible screen
#[derive(PartialEq)]
enum LinkDir {
    Above,
    Below,
    Visible,
}

impl View for Menu {
    fn raw(&self) -> String {
        self.raw.to_string()
    }

    fn render(&self) -> String {
        self.render_lines()
    }

    fn respond(&mut self, key: Key) -> Action {
        self.process_key(key)
    }

    fn term_size(&mut self, cols: usize, rows: usize) {
        self.size = (cols, rows);
    }

    fn url(&self) -> String {
        self.url.to_string()
    }
}

impl Menu {
    pub fn from(url: String, response: String) -> Menu {
        Self::parse(url, response)
    }

    fn link(&self, i: usize) -> Option<&Line> {
        if let Some(line) = self.links.get(i) {
            self.lines.get(*line)
        } else {
            None
        }
    }

    // is the given link visible on the screen right now?
    fn link_visibility(&self, i: usize) -> Option<LinkDir> {
        if let Some(&pos) = self.links.get(i) {
            Some(if pos < self.scroll {
                LinkDir::Above
            } else if pos >= self.scroll + self.size.1 - 1 {
                LinkDir::Below
            } else {
                LinkDir::Visible
            })
        } else {
            None
        }
    }

    fn render_lines(&self) -> String {
        let mut out = String::new();
        let (cols, rows) = self.size;

        macro_rules! push {
            ($c:expr, $e:expr) => {{
                out.push_str("\x1b[");
                out.push_str($c);
                out.push_str("m");
                out.push_str(&$e);
                out.push_str("\x1b[0m");
            }};
        }

        let iter = self.lines.iter().skip(self.scroll).take(rows - 1);
        let longest = if self.longest > MAX_COLS {
            MAX_COLS
        } else {
            self.longest
        };
        let indent = if longest > cols {
            String::from("")
        } else {
            let left = (cols - longest) / 2;
            if left > 6 {
                " ".repeat(left - 6)
            } else {
                String::from("")
            }
        };

        for line in iter {
            if !self.wide {
                out.push_str(&indent);
            }
            if line.typ == Type::Info {
                out.push_str("      ");
            } else {
                if line.link - 1 == self.link {
                    out.push_str("\x1b[97;1m*\x1b[0m")
                } else {
                    out.push(' ');
                }
                out.push(' ');
                out.push_str("\x1b[95m");
                if line.link < 10 {
                    out.push(' ');
                }
                out.push_str(&line.link.to_string());
                out.push_str(".\x1b[0m ");
            }
            // truncate long lines, instead of wrapping
            let name = if line.name.len() > MAX_COLS {
                let mut s = line.name.chars().take(MAX_COLS).collect::<String>();
                s.push_str("...");
                s
            } else {
                line.name.to_string()
            };
            match line.typ {
                Type::Text => push!("96", name),
                Type::Menu => push!("94", name),
                Type::Info => push!("93", name),
                Type::HTML => push!("92", name),
                Type::Error => push!("91", name),
                typ if typ.is_download() => push!("4;97", name),
                _ => push!("0", name),
            }
            out.push('\n');
        }
        out.push_str(&format!(
            "{}{}{}",
            termion::cursor::Goto(1, self.size.1 as u16),
            termion::clear::CurrentLine,
            self.input
        ));
        out
    }

    fn redraw_input(&self) -> Action {
        print!(
            "{}{}{}",
            termion::cursor::Goto(1, self.size.1 as u16),
            termion::clear::CurrentLine,
            self.input
        );
        stdout().flush();
        Action::None
    }

    fn action_page_down(&mut self) -> Action {
        let lines = self.lines.len();
        if lines > SCROLL_LINES && self.scroll < lines - SCROLL_LINES {
            self.scroll += SCROLL_LINES;
            if let Some(dir) = self.link_visibility(self.link) {
                match dir {
                    LinkDir::Above => {
                        let scroll = self.scroll;
                        if let Some(&pos) =
                            self.links.iter().skip(self.link).find(|&&i| i >= scroll)
                        {
                            self.link = self.lines.get(pos).unwrap().link - 1;
                        }
                    }
                    LinkDir::Below => {}
                    LinkDir::Visible => {}
                }
            }
            Action::Redraw
        } else {
            Action::None
        }
    }

    fn action_page_up(&mut self) -> Action {
        if self.scroll > 0 {
            if self.scroll > SCROLL_LINES {
                self.scroll -= SCROLL_LINES;
            } else {
                self.scroll = 0;
            }
            if self.link == 0 {
                return Action::Redraw;
            }
            if let Some(dir) = self.link_visibility(self.link) {
                match dir {
                    LinkDir::Below => {
                        let scroll = self.scroll;
                        if let Some(&pos) = self
                            .links
                            .iter()
                            .take(self.link)
                            .rev()
                            .find(|&&i| i < (self.size.1 + scroll - 2))
                        {
                            self.link = self.lines.get(pos).unwrap().link;
                        }
                    }
                    LinkDir::Above => {}
                    LinkDir::Visible => {}
                }
            }
            Action::Redraw
        } else if self.link > 0 {
            self.link = 0;
            Action::Redraw
        } else {
            Action::None
        }
    }

    fn action_up(&mut self) -> Action {
        if self.link == 0 {
            return if self.scroll > 0 {
                self.scroll -= 1;
                Action::Redraw
            } else {
                Action::None
            };
        }

        let new_link = self.link - 1;
        if let Some(dir) = self.link_visibility(new_link) {
            match dir {
                LinkDir::Above => {
                    // scroll up by 1
                    if self.scroll > 0 {
                        self.scroll -= 1;
                    }
                    // select it if it's visible now
                    if let Some(dir) = self.link_visibility(new_link) {
                        if dir == LinkDir::Visible {
                            self.link = new_link;
                        }
                    }
                }
                LinkDir::Below => {
                    // jump to link....
                    if let Some(&pos) = self.links.get(new_link) {
                        self.scroll = pos;
                        self.link = new_link;
                    }
                }
                LinkDir::Visible => {
                    // select next link up
                    self.link = new_link;
                }
            }
            Action::Redraw
        } else {
            Action::None
        }
    }

    fn action_down(&mut self) -> Action {
        let count = self.links.len();

        // last link selected but there is more content
        if self.lines.len() > self.size.1 + self.scroll - 1 && self.link == count - 1 {
            self.scroll += 1;
            return Action::Redraw;
        }

        if count > 0
            && self.link == count - 1
            && self.lines.len() > self.link
            && self.scroll > SCROLL_LINES
            && count > self.scroll - SCROLL_LINES
        {
            self.scroll += 1;
            return Action::Redraw;
        }

        let new_link = self.link + 1;
        if count > 0 && self.link < count - 1 {
            if let Some(dir) = self.link_visibility(new_link) {
                match dir {
                    LinkDir::Above => {
                        // jump to link....
                        if let Some(&pos) = self.links.get(new_link) {
                            self.scroll = pos;
                            self.link = new_link;
                        }
                    }
                    LinkDir::Below => {
                        // scroll down by 1
                        self.scroll += 1;
                        // select it if it's visible now
                        if let Some(dir) = self.link_visibility(new_link) {
                            if dir == LinkDir::Visible {
                                self.link = new_link;
                            }
                        }
                    }
                    LinkDir::Visible => {
                        // select next link down
                        self.link = new_link;
                    }
                }
                Action::Redraw
            } else {
                Action::None
            }
        } else {
            Action::None
        }
    }

    fn action_select_link(&mut self, link: usize) -> Action {
        if link < self.links.len() {
            if let Some(&line) = self.links.get(link) {
                if self.link_visibility(link) != Some(LinkDir::Visible) {
                    if line > SCROLL_LINES {
                        self.scroll = line - SCROLL_LINES;
                    } else {
                        self.scroll = 0;
                    }
                }
            }
            self.link = link;
            Action::Redraw
        } else {
            Action::None
        }
    }

    fn action_follow_link(&mut self, link: usize) -> Action {
        self.input.clear();
        self.action_select_link(link);
        self.action_open()
    }

    fn action_open(&mut self) -> Action {
        self.input.clear();
        if let Some(line) = self.link(self.link) {
            let url = line.url.to_string();
            let (typ, _, _, _) = gopher::parse_url(&url);
            if typ == Type::Search {
                if let Some(query) = ui::prompt(&format!("{}> ", line.name)) {
                    Action::Open(format!("{}?{}", url, query))
                } else {
                    Action::None
                }
            } else {
                Action::Open(url)
            }
        } else {
            Action::None
        }
    }

    fn process_key(&mut self, key: Key) -> Action {
        match key {
            Key::Char('\n') => self.action_open(),
            Key::Up | Key::Ctrl('p') => self.action_up(),
            Key::Down | Key::Ctrl('n') => self.action_down(),
            Key::Ctrl('w') => {
                self.wide = !self.wide;
                Action::Redraw
            }
            Key::Backspace | Key::Delete => {
                if self.input.is_empty() {
                    Action::Back
                } else {
                    self.input.pop();
                    self.redraw_input()
                }
            }
            Key::Esc => {
                if !self.input.is_empty() {
                    self.input.clear();
                    self.redraw_input()
                } else {
                    Action::None
                }
            }
            Key::Ctrl('c') => {
                if !self.input.is_empty() {
                    self.input.clear();
                    self.redraw_input()
                } else {
                    Action::Quit
                }
            }
            Key::Char('-') => {
                if self.input.is_empty() {
                    self.action_page_up()
                } else {
                    self.input.push('-');
                    self.redraw_input()
                }
            }
            Key::PageUp => self.action_page_up(),
            Key::PageDown => self.action_page_down(),
            Key::Char(' ') => {
                if self.input.is_empty() {
                    self.action_page_down()
                } else {
                    self.input.push(' ');
                    self.redraw_input()
                }
            }
            Key::Char(c) => {
                self.input.push(c);
                let count = self.links.len();
                let input = &self.input;

                // jump to <10 number
                if input.len() == 1 {
                    if let Some(c) = input.chars().nth(0) {
                        if c.is_digit(10) {
                            let i = c.to_digit(10).unwrap() as usize;
                            if i <= count {
                                if count < (i * 10) {
                                    return self.action_follow_link(i - 1);
                                } else {
                                    return self.action_select_link(i - 1);
                                }
                            }
                        }
                    }
                } else if input.len() == 2 {
                    // jump to >=10 number
                    let s = input.chars().take(2).collect::<String>();
                    if let Ok(num) = s.parse::<usize>() {
                        if num <= count {
                            if count < (num * 10) {
                                return self.action_follow_link(num - 1);
                            } else {
                                return self.action_select_link(num - 1);
                            }
                        }
                    }
                } else if input.len() == 3 {
                    // jump to >=100 number
                    let s = input.chars().take(3).collect::<String>();
                    if let Ok(num) = s.parse::<usize>() {
                        if num <= count {
                            if count < (num * 10) {
                                return self.action_follow_link(num - 1);
                            } else {
                                return self.action_select_link(num - 1);
                            }
                        }
                    }
                }

                for i in 0..count {
                    // check for name match
                    let name = if let Some(link) = self.link(i) {
                        link.name.to_ascii_lowercase()
                    } else {
                        "".to_string()
                    };

                    if name.contains(&self.input.to_ascii_lowercase()) {
                        return self.action_select_link(i);
                    }
                }

                // self.link = 0;
                // Action::Redraw
                self.redraw_input()
            }
            _ => Action::Keypress(key),
        }
    }

    // parse gopher response into a Menu object
    pub fn parse(url: String, raw: String) -> Menu {
        let mut lines = vec![];
        let mut links = vec![];
        let mut link = 0;
        let mut longest = 0;
        for line in raw.split_terminator('\n') {
            if let Some(c) = line.chars().nth(0) {
                let typ = match gopher::type_for_char(c) {
                    Some(t) => t,
                    None => continue,
                };

                // assemble line info
                let parts: Vec<&str> = line.split_terminator('\t').collect();

                let mut name = String::from("");
                if !parts[0].is_empty() {
                    name.push_str(&parts[0][1..]);
                }
                if typ != Type::Info {
                    link += 1;
                }
                if name.len() > longest {
                    longest = name.len();
                }
                let link = if typ == Type::Info { 0 } else { link };
                if link > 0 {
                    links.push(lines.len());
                }

                // check for URL:<url> syntax
                if parts.len() > 1 {
                    if parts[1].starts_with("URL:") {
                        lines.push(Line {
                            name,
                            url: parts[1].chars().skip(4).collect::<String>(),
                            typ,
                            link,
                        });
                        continue;
                    }
                }

                // assemble regular, gopher-style URL
                let mut url = String::from("gopher://");
                if parts.len() > 2 {
                    url.push_str(parts[2]); // host
                }
                // port
                if parts.len() > 3 {
                    let port = parts[3].trim_end_matches('\r');
                    if port != "70" {
                        url.push(':');
                        url.push_str(parts[3].trim_end_matches('\r'));
                    }
                }
                // auto-prepend gopher type to selector
                if let Some(first_char) = parts[0].chars().nth(0) {
                    url.push_str("/");
                    url.push(first_char);
                    // add trailing / if the selector is blank
                    if parts.len() == 0 || parts.len() > 1 && parts[1].len() == 0 {
                        url.push('/');
                    }
                }
                if parts.len() > 1 {
                    url.push_str(parts[1]); // selector
                }
                lines.push(Line {
                    name,
                    url,
                    typ,
                    link,
                });
            }
        }

        Menu {
            url,
            lines,
            links,
            longest,
            raw,
            input: String::new(),
            link: 0,
            scroll: 0,
            size: (0, 0),
            wide: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! parse {
        ($s:literal) => {
            Menu::parse("test".to_string(), $s.to_string());
        };
    }

    #[test]
    fn test_simple_menu() {
        let menu = parse!(
            "
i---------------------------------------------------------
1SDF PHLOGOSPHERE (297 phlogs)	/phlogs/	gopher.club	70
1SDF GOPHERSPACE (1303 ACTIVE users)	/maps/	sdf.org	70
i---------------------------------------------------------
"
        );
        assert_eq!(menu.lines.len(), 4);
        assert_eq!(menu.links.len(), 2);
        assert_eq!(menu.lines[1].url, "gopher://gopher.club/1/phlogs/");
        assert_eq!(menu.lines[2].url, "gopher://sdf.org/1/maps/");
    }

    #[test]
    fn test_no_path() {
        let menu = parse!("1Circumlunar Space		circumlunar.space	70");
        assert_eq!(menu.links.len(), 1);
        assert_eq!(menu.lines[0].url, "gopher://circumlunar.space/1/");
    }
}