mod bpftrace_compiler;
mod executor;
mod parser;
mod tui;
mod web;

use std::{
    io::Read,
    panic::{set_hook, take_hook},
};
use tui_textarea::TextArea;
use web::start_server;

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

    //-t to run the tui
    #[clap(short, long)]
    tui: Option<bool>,
}

pub fn init_panic_hook() {
    let original_hook = take_hook();
    set_hook(Box::new(move |panic_info| {
        // intentionally ignore errors here since we're already in a panic
        let _ = tui::restore();
        original_hook(panic_info);
    }));
}

async fn eval_print_string(hostname: String, s: String) -> std::io::Result<()> {
    let ast_result = parser::parse_bpfquery_sql(&s);
    let ast = if let Ok(ast) = ast_result {
        ast
    } else {
        eprintln!("Error parsing expression: {:?}", ast_result);
        return Ok(());
    };
    let (bpf, headers) = bpftrace_compiler::compile_ast_to_bpftrace(ast).unwrap();
    let (results_sender, results_reciver) = tokio::sync::watch::channel([].to_vec());
    dbg!(&headers);
    println!("{}", &bpf);
    let t = tokio::task::spawn(async {
        executor::execute_bpf(hostname, headers, bpf, results_sender).await;
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

    Ok(())
}

async fn read_and_run(filename: String, hostname: String) {
    let mut file = std::fs::File::open(&filename).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    eval_print_string(hostname.clone(), contents.clone()).await;
}
async fn watch_and_run_file(hostname: String, filename: String) -> std::io::Result<()> {
    let mut file_contents = std::fs::read_to_string(&filename).unwrap();

    let f = filename.clone();
    let h = hostname.clone();
    let mut t = tokio::task::spawn(async { read_and_run(f, h).await });

    loop {
        let new_contents = std::fs::read_to_string(&filename).unwrap();
        if file_contents != new_contents {
            file_contents = new_contents;
            let f = filename.clone();
            let h = hostname.clone();
            println!("Killing old thread, save a file change");
            t.abort();
            t = tokio::task::spawn(async { read_and_run(f, h).await });
        }
        // sleep 100ms
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                t.abort();
                break;
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {}
        }
    }
    Ok(())
}

fn start_tui() {
    let textarea =
        TextArea::from(["select str(args.path->dentry->d_name.name) from kprobe.vfs_open"]);

    let mut app = tui::App {
        exit: false,
        counter: 0,
        hostname: "localhost".to_string(),
        textarea,
        bpfoutput: String::new(),
        headers: Vec::new(),
        results: [].to_vec(),
        results_sender: tokio::sync::watch::channel([].to_vec()).0,
        task: tokio::task::spawn(async {}),
    };

    init_panic_hook();
    let mut terminal = tui::init().unwrap();
    let app_result = app.run(&mut terminal);
    tui::restore().unwrap();
}

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();

    if args.expression.is_some() {
        eval_print_string(args.hostname, args.expression.unwrap()).await?;
    } else if args.file.is_some() {
        watch_and_run_file(args.hostname, args.file.unwrap()).await?;
    } else if args.tui.is_some() {
        start_tui();
    }
    else {
        start_server().await;
    }
    return Ok(());
}
