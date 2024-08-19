mod bpftrace_compiler;
mod executor;
mod parser;
mod tui;
use openssh::{KnownHosts, Session};
use tui_textarea::TextArea;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let hostname = &args[1];

    let session = Session::connect(hostname, KnownHosts::Strict)
        .await
        .unwrap();

    let textarea = TextArea::from(["select pid, cpu, elapsed from kprobe.do_nanosleep;"]);

    let mut app = tui::App {
        exit: false,
        counter: 0,
        session,
        hostname: hostname.to_string(),
        textarea,
        bpfoutput: String::new(),
        headers: Vec::new(),
        results: [].to_vec(),
    };

    let mut terminal = tui::init()?;
    let app_result = app.run(&mut terminal);
    tui::restore()?;
    app_result
}
