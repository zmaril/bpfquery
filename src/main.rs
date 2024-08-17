mod bpftrace_compiler;
mod parser;
mod ssh;
use rustyline::error::ReadlineError;
use rustyline::{DefaultEditor, Result};
use std::io::prelude::*;

fn main() -> Result<()> {
    // get first argument of the program and use that as hostname 
    let args: Vec<String> = std::env::args().collect();
    let hostname = &args[1];

    let sql = "select pid, cpu, elapsed from kprobe.do_nanosleep;";
    let sess = ssh::get_session(hostname.to_string());

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
                    _ => &line.clone() 
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

        // send the bpftrace output to the target machine
        let mut channel = sess.channel_session().unwrap();
        channel.exec(&bpftrace_command).unwrap();
        let mut s = String::new();
        channel.read_to_string(&mut s).unwrap();
        dbg!(s);
        channel.wait_close().unwrap();
        dbg!(channel.exit_status().unwrap());
    }
    Ok(())
}
