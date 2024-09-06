use datafusion::arrow::array::{Int32Builder, StringArray, Float32Array, Int32Array};
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
use std::sync::Arc;

use serde_json::Value;
use std::io::Write;
use std::process::Command;
use std::process::Stdio;
use crate::commands::util::get_session;

#[derive(Debug, Clone)]
pub struct Process {
    pub command: String,
    pub cpu_percent: Option<f32>,
    pub mem_percent: Option<f32>,
    pub pid: i32,
    pub rss: i32,
    pub started: Option<String>,
    pub stat: Option<String>,
    pub time: String,
    pub tty: Option<String>,
    pub user: String,
    pub vsz: i32,
}

impl Process {
    pub fn schema() -> SchemaRef {
        let fields = vec![
            Field::new("command", DataType::Utf8, false),
            Field::new("cpu_percent", DataType::Float32, true),
            Field::new("mem_percent", DataType::Float32, true),
            Field::new("pid", DataType::Int32, false),
            Field::new("rss", DataType::Int32, false),
            Field::new("started", DataType::Utf8, true),
            Field::new("stat", DataType::Utf8, true),
            Field::new("time", DataType::Utf8, false),
            Field::new("tty", DataType::Utf8, true),
            Field::new("user", DataType::Utf8, false),
            Field::new("vsz", DataType::Int32, false),
        ];
        Arc::new(Schema::new(fields))
    }
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
            command: p["command"].as_str().unwrap().to_string(),
            cpu_percent: p["cpu_percent"].as_f64().map(|x| x as f32),
            mem_percent: p["mem_percent"].as_f64().map(|x| x as f32),
            pid: p["pid"].as_i64().unwrap() as i32,
            rss: p["rss"].as_i64().unwrap() as i32,
            started: p["started"].as_str().map(|x| x.to_string()),
            stat: p["stat"].as_str().map(|x| x.to_string()),
            time: p["time"].as_str().unwrap().to_string(),
            tty: p["tty"].as_str().map(|tty| tty.to_string()),
            user: p["user"].as_str().unwrap().to_string(),
            vsz: p["vsz"].as_i64().unwrap() as i32,
        })
        .collect();

    processes
}


async fn get_proc_info() -> Result<SendableRecordBatchStream> {
    let session = get_session().await;


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
        let user_array = StringArray::from(vec![process.user]);
        let vsz_array = Int32Array::from(vec![process.vsz]);
        let rss_array = Int32Array::from(vec![process.rss]);
        let tty_array = StringArray::from(vec![process.tty.unwrap_or("".to_string())]);
        let stat_array = StringArray::from(vec![process.stat.unwrap_or("".to_string())]);
        let started_array = StringArray::from(vec![process.started.unwrap_or("".to_string())]);
        let time_array = StringArray::from(vec![process.time]);
        let cpu_percent_array = Float32Array::from(vec![process.cpu_percent.unwrap_or(0.0)]);
        let mem_percent_array = Float32Array::from(vec![process.mem_percent.unwrap_or(0.0)]);
        let batch = RecordBatch::try_new(
            Process::schema(),
            vec![
                Arc::new(command_array),
                Arc::new(cpu_percent_array),
                Arc::new(mem_percent_array),
                Arc::new(pid_array.finish()),
                Arc::new(rss_array),
                Arc::new(started_array),
                Arc::new(stat_array),
                Arc::new(time_array),
                Arc::new(tty_array),
                Arc::new(user_array),
                Arc::new(vsz_array),
            ],
        )?;
        batches.push(Ok(batch));
    }


    let stream = stream::iter(batches);

    Ok(Box::pin(RecordBatchStreamAdapter::new(
        Process::schema(),
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
        Process::schema()
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