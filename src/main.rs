mod bpftrace_compiler;
mod executor;
mod parser;
mod tui;
use tui_textarea::TextArea;

use std::{
    io::{self, stdout},
    panic::{set_hook, take_hook},
    thread::sleep,
    time::Duration,
};

pub fn init_panic_hook() {
    let original_hook = take_hook();
    set_hook(Box::new(move |panic_info| {
        // intentionally ignore errors here since we're already in a panic
        let _ = tui::restore();
        original_hook(panic_info);
    }));
}

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let hostname = &args[1];

    let textarea = TextArea::from(["select comm, pid, cpu, elapsed from kprobe.do_nanosleep;"]);

    let mut app = tui::App {
        exit: false,
        counter: 0,
        hostname: hostname.to_string(),
        textarea,
        bpfoutput: String::new(),
        headers: Vec::new(),
        results: [].to_vec(),
        results_sender: tokio::sync::watch::channel([].to_vec()).0,
        task: tokio::task::spawn(async {}),
    };

    init_panic_hook();
    let mut terminal = tui::init()?;
    let app_result = app.run(&mut terminal).await;
    tui::restore()?;
    app_result
}
