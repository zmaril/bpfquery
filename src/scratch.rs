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
use futures::stream::{self, StreamExt};
use openssh::{KnownHosts, SessionBuilder};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::sync::Arc;

#[derive(Clone, Debug)]
struct CustomExec {
    cache: PlanProperties,
    projected_schema: SchemaRef,
    db: ProcessTable,
}

async fn get_session() -> openssh::Session {
    let mut s = SessionBuilder::default();
    let mut h = std::env::var("BPFTRACE_MACHINE").unwrap();
    let user = "root".to_string();
    h = format!("{}@{}", user, h);
    s.keyfile("/app/bpftrace_machine");
    s.known_hosts_check(KnownHosts::Accept);
    s.connect(h).await.unwrap()
}

async fn get_procs(session: &openssh::Session) -> Vec<i32> {
    let mut cmd = session.command("ls");
    cmd.arg("/proc");
    let output = cmd.output().await.unwrap();
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .filter(|s| s.parse::<i32>().is_ok())
        .map(|s| s.parse::<i32>().unwrap())
        .collect::<Vec<i32>>()
}

async fn read_remote_file(session: &openssh::Session, path: String) -> String {
    let mut cmd = session.command("cat");
    cmd.arg(path);
    let output = cmd.output().await.unwrap();
    String::from_utf8(output.stdout).unwrap()
}

async fn get_proc_info() -> Result<SendableRecordBatchStream> {
    let session = get_session().await;
    let pids = get_procs(&session).await;

    let mut batches = Vec::new();
    for pid in pids {
        let fields = vec![
            Field::new("pid", DataType::Int32, false),
            Field::new("comm", DataType::Utf8, false),
            Field::new("cmdline", DataType::Utf8, false),
        ];
        let schema = Schema::new(fields);
        
        let mut pid_array = Int32Builder::new();
        pid_array.append_value(pid);

        let comm = read_remote_file(&session, format!("/proc/{}/comm", pid)).await;
        let cmdline = read_remote_file(&session, format!("/proc/{}/cmdline", pid)).await;

        let comm_array = StringArray::from(vec![comm]);
        let cmdline_array = StringArray::from(vec![cmdline]);

        let batch = RecordBatch::try_new(
            Arc::new(schema.clone()),
            vec![
                Arc::new(pid_array.finish()),
                Arc::new(comm_array),
                Arc::new(cmdline_array),
            ],
        )?;

        batches.push(Ok(batch));
    }

    let stream = stream::iter(batches);

    let fields = vec![
        Field::new("pid", DataType::Int32, false),
        Field::new("comm", DataType::Utf8, false),
        Field::new("cmdline", DataType::Utf8, false),
    ];
    let schema = Schema::new(fields);

    Ok(Box::pin(RecordBatchStreamAdapter::new(
        Arc::new(schema),
        stream,
    )))
}

impl CustomExec {
    fn new(projections: Option<&Vec<usize>>, schema: SchemaRef, db: ProcessTable) -> Self {
        let projected_schema = project_schema(&schema, projections).unwrap();
        let cache = Self::compute_properties(projected_schema.clone());
        Self {
            db,
            projected_schema,
            cache,
        }
    }

    /// This function creates the cache object that stores the plan properties such as schema, equivalence properties, ordering, partitioning, etc.
    fn compute_properties(schema: SchemaRef) -> PlanProperties {
        let eq_properties = EquivalenceProperties::new(schema);
        PlanProperties::new(
            eq_properties,
            Partitioning::UnknownPartitioning(1),
            ExecutionMode::Bounded,
        )
    }
}

impl DisplayAs for CustomExec {
    fn fmt_as(&self, _t: DisplayFormatType, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "CustomExec")
    }
}

impl ExecutionPlan for CustomExec {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn with_new_children(
        self: Arc<Self>,
        _: Vec<Arc<dyn ExecutionPlan>>,
    ) -> Result<Arc<dyn ExecutionPlan>> {
        Ok(self)
    }

    fn name(&self) -> &str {
        "CustomExec"
    }

    fn execute(
        &self,
        _partition: usize,
        _context: Arc<TaskContext>,
    ) -> Result<SendableRecordBatchStream> {
        // ssh into the machine and process the /proc directory
        // return a stream of record batches
        let fut = get_proc_info();
        let stream = futures::stream::once(fut).try_flatten();
        let schema = self.schema().clone();
        let b = Box::pin(RecordBatchStreamAdapter::new(schema, stream));
        Ok(b)
    }

    fn properties(&self) -> &datafusion::physical_plan::PlanProperties {
        &self.cache
    }

    fn children(&self) -> Vec<&Arc<dyn ExecutionPlan>> {
        vec![]
    }
}

#[derive(Debug, Clone)]
struct ProcessTable {}

#[async_trait]
impl TableProvider for ProcessTable {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn schema(&self) -> Arc<Schema> {
        let schema = Schema::new(vec![
            Field::new("pid", DataType::Int32, false),
            Field::new("name", DataType::Utf8, false),
            Field::new("cmdline", DataType::Utf8, false),
        ]);
        Arc::new(schema)
    }
    fn table_type(&self) -> TableType {
        TableType::Base
    }
    async fn scan(
        &self,
        _state: &dyn datafusion::catalog::Session,
        projection: Option<&Vec<usize>>,
        _filters: &[Expr],
        _limit: Option<usize>,
    ) -> datafusion::common::Result<Arc<dyn ExecutionPlan>> {
        Ok(Arc::new(CustomExec::new(
            projection,
            self.schema(),
            self.clone(),
        )))
    }
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
