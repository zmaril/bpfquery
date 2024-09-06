use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use datafusion::execution::context::SessionContext;
use std::sync::Arc;

use crate::commands::ps::ProcessTable;

async fn set_up() -> std::io::Result<SessionContext> {
    let ctx = SessionContext::new();
    let process_table = Arc::new(ProcessTable {});
    ctx.register_table("process", process_table).unwrap();
    Ok(ctx)
}

async fn eval_sql(ctx: &SessionContext, sql: String) -> std::io::Result<()> {
    let result = ctx.sql(&sql).await;
    match result {
        Ok(df) => {
            df.show().await.unwrap();
        }
        Err(e) => {
            println!("Error: {:?}", e);
        }
    }
    Ok(())
}
pub async fn cli_eval(sql: String) -> std::io::Result<()> {
    let ctx = set_up().await.unwrap();
    eval_sql(&ctx, sql).await.unwrap();
    Ok(())
}

pub async fn cli_repl() -> std::io::Result<()> {
    let ctx = set_up().await.unwrap();

    let mut rl = DefaultEditor::new().unwrap();
    if rl.load_history("history.txt").is_err() {
        println!("No previous history.");
    }
    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str()).unwrap();
                eval_sql(&ctx, line).await.unwrap();
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
        }
    }
    rl.save_history("history.txt").unwrap();
    Ok(())
}
