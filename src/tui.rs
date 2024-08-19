use openssh::Session;
use ratatui::prelude::*;
use std::{collections::HashMap, io::{self, stdout, Stdout}};
use tui_textarea::TextArea;
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

use crate::bpftrace_compiler::compile_ast_to_bpftrace;
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
    pub session: Session,
    pub hostname: String,
    pub textarea: TextArea<'static>,
    pub bpfoutput: String,
    pub headers: Vec<String>,
    pub results: Vec<HashMap<String,String>>
}

impl App {
    pub fn run(&mut self, terminal: &mut Tui) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.render_frame(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn render_frame(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            // it's important to check that the event is a key press event as
            // crossterm also emits key release and repeat events on Windows.
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            _ => {}
        };
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if key_event.code == KeyCode::Esc {
            self.exit();
        } else {
            self.textarea.input(key_event);
            let s = self.textarea.lines().join("\n");
            let ast_result = parse_bpfquery_sql(&s); 
            if let Ok(ast) = ast_result {
                let (bpfoutput, headers) = compile_ast_to_bpftrace(ast);
                self.bpfoutput = bpfoutput;
                self.headers = headers;
            }
            else {
                self.bpfoutput = "Error parsing sql\n".to_string();
                self.bpfoutput.push_str(&ast_result.unwrap_err().to_string());
            }
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

        let widths = self.headers.iter().map(|_| Constraint::Percentage(100 / self.headers.len() as u16)).collect::<Vec<_>>();

        // convert the results into rows
        let mut rows = Vec::new();
        for result in self.results.clone() {
            let mut row = Vec::new();
            for header in self.headers.clone() {
                let value = result[header.as_str()].to_string();
                row.push(value);
            }
            rows.push(Row::new(row));
        }


        let table = Table::new(rows, widths)
            // ...and they can be separated by a fixed spacing.
            .column_spacing(1)
            // It has an optional header, which is simply a Row always visible at the top.
            .header(heading)
            // As any other widget, a Table can be wrapped in a Block.
            .block(Block::new().title("bpftrace results"));

        ratatui::prelude::Widget::render(&table, right, buf);
    }
}
