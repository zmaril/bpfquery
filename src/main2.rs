use openssh::{KnownHosts, Session, Stdio};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::signal::ctrl_c;
use home::home_dir;
use std::collections::HashMap;


#[tokio::main]
async fn main() -> Result<()> {
    // get first argument of the program and use that as hostname
    let args: Vec<String> = std::env::args().collect();
    let hostname = &args[1];

    let sql = "select pid, cpu, elapsed from kprobe.do_nanosleep;";
    let session = Session::connect(hostname, KnownHosts::Strict)
        .await
        .unwrap();

    // make a repl for the user to input sql queries and have them be compiled into bpftrace
    // and then run on the target machine

    // `()` can be used when no completer is required
    let mut rl = DefaultEditor::new()?;


    println!("Welcome to bpfquery, the crossroads of sql and bpf(trace)! Type 'exit' to exit, 'go' to run the default query, type your own SQL query. `help` has more info.");
    //Make a .bpfquery directory in the home dir 
    let home = home_dir().unwrap();
    let bpfquery_dir = home.join(".bpfquery");
    std::fs::create_dir_all(&bpfquery_dir).unwrap();
    let history_file = bpfquery_dir.join("history.txt");
    if rl.load_history(history_file.as_path()).is_err() {
        println!("No previous history file found, starting a new one.");
    }

    loop {
        let readline = rl.readline(format!("bpfquery/{}> ", hostname).as_str());
        let line = match readline {
            Ok(line) => {
                rl.add_history_entry(line.clone().as_str()).unwrap();
                match line.as_str() {
                    "exit" => break,
                    "help" => {
                        println!("Type 'exit' to exit, 'go' to run the default query, type your own SQL query to have that run. You can also use the arrow keys to navigate the history of your shell. Press 'CTRL-C' to stop a bpftrace from running.");
                        continue;
                    }
                    "go" => sql,
                    _ => &line.clone(),
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        };
        execute_sql(&session, line).await;
    }
    rl.save_history(history_file.as_path()).unwrap();
    Ok(())
}
