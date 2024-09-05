use rustyline::error::ReadlineError;
use rustyline::{DefaultEditor, Result};
use datafusion::prelude::*;

pub async fn go() -> Result<()> {
    let ctx = SessionContext::new();
    ctx.register_csv("airtravel", "airtravel.csv", CsvReadOptions::new()).await.unwrap();

    let mut rl = DefaultEditor::new()?;
    if rl.load_history("history.txt").is_err() {
        println!("No previous history.");
    }
    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str()).unwrap();
                let r = ctx.sql(line.as_str()).await;
                match r {
                    Ok(df) => {
                        df.show().await.unwrap();
                    }
                    Err(e) => {
                        println!("Error: {:?}", e);
                    }
                }
            },
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break
            },
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break
            },
            Err(err) => {
                println!("Error: {:?}", err);
                break
            }
        }
    }
    rl.save_history("history.txt").unwrap();
    Ok(())
}