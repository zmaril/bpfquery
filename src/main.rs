mod scratch;

use dotenv::dotenv;
use scratch::go;

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
}

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> std::io::Result<()> {
    let _args = Args::parse();
    dotenv().ok();

    go();

    return Ok(());
}
