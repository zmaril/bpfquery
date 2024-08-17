mod bpftrace_compiler;
mod parser;
use openssh::{KnownHosts, Session, Stdio};
use rustyline::error::ReadlineError;
use rustyline::{DefaultEditor, Result};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};

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
    loop {
        let readline = rl.readline(format!("bpfquery/{}> ", hostname).as_str());
        let line = match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str()).unwrap();
                match line.as_str() {
                    "exit" => break,
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
        let ast = parser::parse_bpfquery_sql(&line);
        let bpftrace_output = bpftrace_compiler::compile_ast_to_bpftrace(ast);
        // actually run the bpftrace output on the target machine
        let mut bpftrace_command = format!("bpftrace -e '{}'", bpftrace_output);
        println!("{}", bpftrace_command);

        let mut remote_cmd = session.command("bpftrace");
        remote_cmd.arg("-e");
        remote_cmd.arg(bpftrace_output);
        remote_cmd.stdout(Stdio::piped());
        
        let mut handle = remote_cmd.spawn().await.unwrap();
        let stdout = handle.stdout().as_mut().unwrap();

        let stdout_reader = BufReader::new(stdout);

        let mut lines = stdout_reader.lines();
        while let Some(line) = lines.next_line().await.unwrap() {
            println!("{}", line);
        }

        




    }
    Ok(())
}
