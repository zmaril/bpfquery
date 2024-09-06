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

use serde_json::Value;
use std::io::Write;
use std::process::Command;
use std::process::Stdio;
use crate::commands::util::get_session;

#[derive(Debug, Clone)]
pub struct Process {
    pub user: String,
    pub pid: i32,
    pub vsz: i32,
    pub rss: i32,
    pub tty: Option<String>,
    pub stat: Option<String>,
    pub started: Option<String>,
    pub time: String,
    pub command: String,
    pub cpu_percent: Option<f32>,
    pub mem_percent: Option<f32>,
}

pub fn parse(ps_output: String) -> Vec<Process> {
    let mut cmd = Command::new("jc");
    cmd.arg("--ps");

    //pipe in the ps output
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    let mut child = cmd.spawn().expect("Failed to execute command");
    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    stdin
        .write_all(ps_output.as_bytes())
        .expect("Failed to write to stdin");
    drop(stdin); // Drop the stdin handle to allow the child process to exit

    let output = child.wait_with_output().expect("Failed to read output");
    let jc_output = String::from_utf8(output.stdout).expect("Failed to read output");

    let json: Value = serde_json::from_str(&jc_output).expect("Failed to parse JSON");

    let processes = json
        .as_array()
        .unwrap()
        .iter()
        .map(|p| Process {
            pid: p["pid"].as_i64().unwrap() as i32,
            command: p["command"].as_str().unwrap().to_string(),
            user: p["user"].as_str().unwrap().to_string(),
            vsz: p["vsz"].as_i64().unwrap() as i32,
            rss: p["rss"].as_i64().unwrap() as i32,
            tty: p["tty"].as_str().map(|tty| tty.to_string()),
            stat: p["stat"].as_str().map(|x| x.to_string()),
            started: p["started"].as_str().map(|x| x.to_string()),
            time: p["time"].as_str().unwrap().to_string(),
            cpu_percent: p["cpu_percent"].as_f64().map(|x| x as f32),
            mem_percent: p["mem_percent"].as_f64().map(|x| x as f32),
        })
        .collect();

    processes
}


async fn get_proc_info() -> Result<SendableRecordBatchStream> {
    let session = get_session().await;

    let fields = vec![
        Field::new("pid", DataType::Int32, false),
        Field::new("command", DataType::Utf8, false),
    ];
    let schema = Schema::new(fields);

    //just run ps aux and get the output
    let ps_aux = session.command("ps").arg("aux").output().await.unwrap();
    let ps_aux_str = String::from_utf8(ps_aux.stdout).unwrap();

    //run it through jc --ps locally on the cli
    let jc_ps = parse(ps_aux_str);


    let mut batches = Vec::new();

    for process in jc_ps {
        let mut pid_array = Int32Builder::new();
        pid_array.append_value(process.pid);

        let command_array = StringArray::from(vec![process.command]);

        let batch = RecordBatch::try_new(
            Arc::new(schema.clone()),
            vec![
                Arc::new(pid_array.finish()),
                Arc::new(command_array),
            ],
        )?;
        batches.push(Ok(batch));
    }


    let stream = stream::iter(batches);

    Ok(Box::pin(RecordBatchStreamAdapter::new(
        Arc::new(schema),
        stream,
    )))
}


#[derive(Clone, Debug)]
struct CustomExec {
    cache: PlanProperties,
    //projected_schema: SchemaRef,
    //db: ProcessTable,
}


impl CustomExec {
    fn new(projections: Option<&Vec<usize>>, schema: SchemaRef, _db: ProcessTable) -> Self {
        let projected_schema = project_schema(&schema, projections).unwrap();
        let cache = Self::compute_properties(projected_schema.clone());
        Self {
            //db,
            //projected_schema,
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
pub struct ProcessTable {}

#[async_trait]
impl TableProvider for ProcessTable {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn schema(&self) -> Arc<Schema> {
        let schema = Schema::new(vec![
            Field::new("pid", DataType::Int32, false),
            Field::new("command", DataType::Utf8, false),
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