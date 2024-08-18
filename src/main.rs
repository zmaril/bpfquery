mod bpftrace_compiler;
mod parser;
use openssh::{KnownHosts, Session, Stdio};
use rustyline::error::ReadlineError;
use rustyline::{DefaultEditor, Result};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::signal::ctrl_c;
use home::home_dir;

async fn execute_sql(session: &Session,sql: &str) {
    println!("Executing SQL: {}", sql);
    let ast = parser::parse_bpfquery_sql(sql);
    let bpftrace_output = bpftrace_compiler::compile_ast_to_bpftrace(ast);
    // actually run the bpftrace output on the target machine
    let bpftrace_command = format!("bpftrace -e '{}'", bpftrace_output);
    println!("{}", bpftrace_command);

    let mut remote_cmd = session.command("bpftrace");
    remote_cmd.arg("-e");
    remote_cmd.arg(bpftrace_output);
    remote_cmd.stdout(Stdio::piped());

    let mut handle = remote_cmd.spawn().await.unwrap();
    let stdout = handle.stdout().as_mut().unwrap();

    let stdout_reader = BufReader::new(stdout);

    let mut lines = stdout_reader.lines();

    // Use `select!` to wait for either Ctrl-C or the next line
    loop {
        tokio::select! {
            _ = ctrl_c() => {
                break;
            }
            line = lines.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        println!("{}", line);
                    }
                    Ok(None) => break, // End of stream
                    Err(e) => {
                        eprintln!("Error reading line: {}", e);
                        break;
                    }
                }
            }
        }
    }
}
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
