use ratatui::prelude::*;
use ratatui::{
    backend::CrosstermBackend,
    buffer::Buffer,
    crossterm::{
        event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    layout::{Alignment, Rect},
    style::Stylize,
    symbols::border,
    text::{Line, Text},
    widgets::{
        block::{Position, Title},
        Block, Paragraph, Row, Table, Widget,
    },
    Frame, Terminal,
};
use std::io::{self, stdout, Stdout};
use tokio::sync::watch;
use tokio::task;
use tui_textarea::TextArea;
use serde_json::Value;

use crate::bpftrace_compiler::compile_ast_to_bpftrace;
use crate::executor::execute_bpf;
use crate::parser::parse_bpfquery_sql;

pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Initialize the terminal
pub fn init() -> io::Result<Tui> {
    execute!(stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    Terminal::new(CrosstermBackend::new(stdout()))
}

/// Restore the terminal to its original state
pub fn restore() -> io::Result<()> {
    execute!(stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}

#[derive(Debug)]
pub struct App {
    pub exit: bool,
    pub counter: u32,
    pub hostname: String,
    pub textarea: TextArea<'static>,
    pub bpfoutput: String,
    pub headers: Vec<String>,
    pub results: Vec<Vec<Value>>,
    pub results_sender: watch::Sender<Vec<Vec<Value>>>,
    pub task: task::JoinHandle<()>,
}

impl App {
    pub async fn run(&mut self, terminal: &mut Tui) -> io::Result<()> {
        let (results_sender, rx) = watch::channel(self.results.clone());
        self.results_sender = results_sender;

        self.update_sql();

        loop {
            if self.exit {
                break;
            }
            terminal.draw(|frame| self.render_frame(frame))?;
            // select! for results recievere and for handling events
            self.handle_events()?;
            let data = rx.borrow().clone();
            if self.results != *data {
                self.results = data.clone();
            }
        }
        Ok(())
    }

    fn render_frame(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn handle_events(&mut self) -> io::Result<()> {
        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                // it's important to check that the event is a key press event as
                // crossterm also emits key release and repeat events on Windows.
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    self.handle_key_event(key_event)
                }
                _ => {}
            };
        }
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if key_event.code == KeyCode::Esc {
            self.exit();
        } else {
            self.textarea.input(key_event);
            self.update_sql();
        }
    }

    fn update_sql(&mut self) {
        let s = self.textarea.lines().join("\n");
        //let formatted = sqlformat::format(&s, &sqlformat::QueryParams::None, sqlformat::FormatOptions { indent: sqlformat::Indent::Spaces(2), uppercase: true, lines_between_queries: 1 });
        let ast_result = parse_bpfquery_sql(&s);
        // set text area to be formatted
        //let cursor = self.textarea.cursor();
        // self.textarea.select_all();
        // self.textarea.cut();
        // self.textarea.insert_str(&formatted);
        // self.textarea.move_cursor(CursorMove::Jump(cursor.0 as u16, cursor.1 as u16));

        if let Ok(ast) = ast_result {
            let r = compile_ast_to_bpftrace(ast);
            if let Ok((bpfoutput, headers)) = r {
                if bpfoutput != self.bpfoutput {
                    self.bpfoutput = bpfoutput;
                    self.headers = headers;

                    let h = self.hostname.clone();
                    let he = self.headers.clone();
                    let b = self.bpfoutput.clone();
                    let results_sender = self.results_sender.clone();

                    self.task.abort();
                    results_sender.send(vec![]).unwrap();

                    self.task = task::spawn(async {
                        execute_bpf(h, he, b, results_sender).await;
                    });
                }
            } else {
                self.bpfoutput = "Error compiling sql:\n".to_string();
                self.bpfoutput.push_str(r.unwrap_err());
            }
        } else {
            self.bpfoutput = "Error parsing sql\n".to_string();
            self.bpfoutput
                .push_str(&ast_result.unwrap_err().to_string());
        }
    }

    fn exit(&mut self) {
        self.exit = true;
        self.task.abort();
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        //Make the large container
        let title = Title::from(format!(" bpfquery/{} ", self.hostname).bold());
        let instructions = Title::from(Line::from(vec![
            " Type your sql query on the left and the results will be streamed out on the right. "
                .into(),
            "<Esc>".blue().bold(),
            " to quit ".into(),
        ]));

        let block = Block::bordered()
            .title(title.alignment(Alignment::Center))
            .title(
                instructions
                    .alignment(Alignment::Center)
                    .position(Position::Bottom),
            )
            .border_set(border::THICK);

        block.clone().render(area, buf);

        //Split the container into 3 parts
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Percentage(33),
                Constraint::Percentage(33),
                Constraint::Percentage(33),
            ])
            .spacing(1)
            .split(block.inner(area));

        let left = layout[0];
        let middle = layout[1];
        let right = layout[2];

        // how to add a divideder between the left and middle

        // Make the sql editor on the left
        self.textarea.render(left, buf);

        // Make the bpfoutput in the middle
        let bpfoutput = Paragraph::new(Text::from(self.bpfoutput.clone()))
            .alignment(Alignment::Left)
            .wrap(ratatui::widgets::Wrap { trim: true });

        bpfoutput.render(middle, buf);

        // Make the table on the right
        if self.results.len() == 1 && self.results[0].len() == 1 {
            //Display a paragraph with the error
            let s = self.results[0][0].to_string();
            let error = Paragraph::new(Text::from(s))
                .alignment(Alignment::Left)
                .wrap(ratatui::widgets::Wrap { trim: true });
            error.render(right, buf);
        } else {
            // Put the headers in there
            let mut headers = ["id".to_string()].to_vec();
            for header in self.headers.clone() {
                headers.push(header);
            }
            let heading = Row::new(headers.clone());

            let widths = headers
                .iter()
                .map(|_| Constraint::Percentage(100 / headers.len() as u16))
                .collect::<Vec<_>>();

            // convert the results into rows
            let mut rows = Vec::new();
            for result in self.results.clone() {
                let s = result
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>();
                rows.push(Row::new(s));
            }

            let table = Table::new(rows, widths)
                // ...and   they can be separated by a fixed spacing.
                .column_spacing(1)
                // It has an optional header, which is simply a Row always visible at the top.
                .header(heading)
                // As any other widget, a Table can be wrapped in a Block.
                .block(Block::new().title("bpftrace results"));

            ratatui::prelude::Widget::render(&table, right, buf);
        }
    }
}
