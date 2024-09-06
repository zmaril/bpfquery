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

pub async fn get_session() -> openssh::Session {
    let mut s = SessionBuilder::default();
    let mut h = std::env::var("BPFTRACE_MACHINE").unwrap();
    let user = "root".to_string();
    h = format!("{}@{}", user, h);
    s.keyfile("bpftrace_machine");
    s.known_hosts_check(KnownHosts::Accept);
    s.connect(h).await.unwrap()
}
