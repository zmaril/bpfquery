mod bpftrace_compiler;
mod executor;
mod parser;
mod tui;
use std::panic::{set_hook, take_hook};
use tui_textarea::TextArea;

use clap::Parser;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(
    name = "bpfquery",
    version = "0.1",
    author = "Zack Maril <zack@zacharymaril.com>",
    about = "An experiment with sql and bpf."
)]
struct Args {
    hostname: String,

    //-e to evaluate an expression
    #[clap(short, long)]
    expression: Option<String>,

    //-f to evaluate a file
    #[clap(short, long)]
    file: Option<String>,
}

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
    let args = Args::parse();

    if args.expression.is_some() {
        let ast = parser::parse_bpfquery_sql(&args.expression.unwrap()).unwrap();
        let (bpf, headers) = bpftrace_compiler::compile_ast_to_bpftrace(ast).unwrap();
        let (results_sender, results_reciver) = tokio::sync::watch::channel([].to_vec());

        let t = tokio::task::spawn(async {
            executor::execute_bpf(args.hostname, headers, bpf, results_sender).await;
        });

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    break;
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {
                    let data = results_reciver.borrow().clone();
                    if data.len() == 1 && data[0].len() == 1 {
                        println!("{}", data[0][0]);
                        break;
                    }
                    else if !data.is_empty() && data[data.len()-1].len() == 1 && data[data.len()-1][0] == "DONE" {
                        for i in 0..data.len()-1 {
                            println!("{}", data[i][1..].into_iter().map(|x| x.to_string()).collect::<Vec<String>>().

                                join(", "));
                        }
                        break;
                    }
                    println!("{:?}", data);
                }
            }
        }

        return Ok(());
    } else if args.file.is_some() {
        println!("wip");
        return Ok(());
    } else {
        let textarea = TextArea::from(["select comm, pid, cpu, elapsed from kprobe.do_nanosleep;"]);

        let mut app = tui::App {
            exit: false,
            counter: 0,
            hostname: args.hostname.to_string(),
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
        return app_result;
    }
}
