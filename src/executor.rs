use home::home_dir;
use openssh::{KnownHosts, Session, Stdio};
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::signal::ctrl_c;

use sqlparser::ast::*;

use crate::bpftrace_compiler;
use crate::parser;

async fn execute_sql(session: &Session, sql: &str) {
    let ast = parser::parse_bpfquery_sql(sql).unwrap();
    let (bpftrace_output, headers) = bpftrace_compiler::compile_ast_to_bpftrace(ast);

    let mut remote_cmd = session.command("bpftrace");
    remote_cmd.arg("-f");
    remote_cmd.arg("json");
    remote_cmd.arg("-e");
    remote_cmd.arg(bpftrace_output);
    remote_cmd.stdout(Stdio::piped());

    let mut handle = remote_cmd.spawn().await.unwrap();
    let stdout = handle.stdout().as_mut().unwrap();

    let stdout_reader = BufReader::new(stdout);

    let mut lines = stdout_reader.lines();

    // Use `select!` to wait for either Ctrl-C or the next line
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                //parse json
                let v: serde_json::Value = serde_json::from_str(&line).unwrap();
                if v["type"] == "attached_probes" {
                    continue;
                }
                //convert array of key value pairs to dict

                let mut d = HashMap::new();
                let mut id = 0;

                for kv in v["data"].as_array().unwrap() {
                    let k = kv[0].as_str().unwrap();
                    let v = &kv[1];
                    if k == "id" {
                        id = v.as_u64().unwrap();
                    } else {
                        d.insert(k, v.clone());
                    }
                }

                let mut row = Vec::new();
                //put the id in
                row.push(id.to_string());
                for header in headers.clone() {
                    let value = d[header.as_str()].to_string();
                    row.push(value);
                }
                println!("{:?}", row);
            }
            Ok(None) => break, // End of stream
            Err(e) => {
                eprintln!("Error reading line: {}", e);
                break;
            }
        }
    }
}
