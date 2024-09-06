use datafusion::arrow::array::{Int32Builder, StringArray};
use datafusion::arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::datasource::{TableProvider, TableType};
use datafusion::error::Result;
use datafusion::execution::context::TaskContext;
use datafusion::physical_expr::EquivalenceProperties;
use datafusion::physical_plan::stream::RecordBatchStreamAdapter;
use datafusion::physical_plan::{
    project_schema, DisplayAs, DisplayFormatType, ExecutionMode, ExecutionPlan, Partitioning,
    PlanProperties, SendableRecordBatchStream,
};
use datafusion::prelude::*;
use futures::TryStreamExt;
use std::any::Any;

use async_trait::async_trait;
use futures::stream::{self};
use openssh::{KnownHosts, SessionBuilder};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::sync::Arc;

use crate::commands::ps::ProcessTable;

#[derive(Clone, Debug)]
struct CustomExec {
    cache: PlanProperties,
    //projected_schema: SchemaRef,
    //db: ProcessTable,
}

async fn get_session() -> openssh::Session {
    let mut s = SessionBuilder::default();
    let mut h = std::env::var("BPFTRACE_MACHINE").unwrap();
    let user = "root".to_string();
    h = format!("{}@{}", user, h);
    s.keyfile("bpftrace_machine");
    s.known_hosts_check(KnownHosts::Accept);
    s.connect(h).await.unwrap()
}

pub async fn set_up() -> std::io::Result<SessionContext> {
    let ctx = SessionContext::new();
    ctx.register_csv("airtravel", "airtravel.csv", CsvReadOptions::new())
        .await
        .unwrap();
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
