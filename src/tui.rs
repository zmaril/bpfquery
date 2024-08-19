use openssh::Session;
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
use std::result;
use std::sync::{Arc, Mutex};
use std::{
    collections::HashMap,
    io::{self, stdout, Stdout},
};
use tui_textarea::TextArea;

use crate::bpftrace_compiler::compile_ast_to_bpftrace;
use crate::executor::execute_sql;
use crate::parser::parse_bpfquery_sql;
use tokio::sync::watch;
use tokio::task;

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
    pub results: Vec<Vec<String>>,
}

impl App {
    pub async fn run(&mut self, terminal: &mut Tui) -> io::Result<()> {
        self.update_sql();
        let (results_sender, results_reciever) = watch::channel(self.results.clone());

        let h = self.hostname.clone();
        let he = self.headers.clone();
        let b = self.bpfoutput.clone();

        task::spawn(async {
            println!("executing sql");
            execute_sql(h, he, b, results_sender).await;
        });

        loop {
            if self.exit {
                break;
            }
            terminal.draw(|frame| self.render_frame(frame))?;
            // select! for results recievere and for handling events
            self.handle_events()?;
            let data = results_reciever.borrow().clone();
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
        let ast_result = parse_bpfquery_sql(&s);
        if let Ok(ast) = ast_result {
            let (bpfoutput, headers) = compile_ast_to_bpftrace(ast);
            self.bpfoutput = bpfoutput;
            self.headers = headers;
        } else {
            self.bpfoutput = "Error parsing sql\n".to_string();
            self.bpfoutput
                .push_str(&ast_result.unwrap_err().to_string());
        }
    }

    fn exit(&mut self) {
        self.exit = true;
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

        // Put the headers in there
        let heading = Row::new(self.headers.clone());

        let widths = self
            .headers
            .iter()
            .map(|_| Constraint::Percentage(100 / self.headers.len() as u16))
            .collect::<Vec<_>>();

        // convert the results into rows
        let mut rows = Vec::new();
        for result in self.results.clone() {
            rows.push(Row::new(result));
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
