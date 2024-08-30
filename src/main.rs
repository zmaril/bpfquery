mod bpftrace_compiler;
mod executor;
mod parser;
mod web;

use web::start_server;
use dotenv::dotenv;

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
    #[arg(short, long)]
    demo: bool,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();
    dotenv().ok();

    start_server(args.hostname, args.demo).await;
    return Ok(());
}
