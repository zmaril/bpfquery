mod scratch;
mod commands;

use dotenv::dotenv;
use scratch::{cli_eval, cli_repl};

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
    #[arg(short, long)]
    demo: bool,

    #[arg(short, long)]
    eval: Option<String>,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();
    dotenv().ok();

    if args.eval.is_some() {
        cli_eval(args.eval.unwrap()).await.unwrap();
    } else {
        cli_repl().await.unwrap();
    }

    Ok(())
}
