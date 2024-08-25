use home::env;
use openssh::{KnownHosts, Session, SessionBuilder, Stdio};
use serde_json::Value;
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::watch;
use whoami;

pub async fn execute_bpf_locally(
    headers: Vec<String>,
    bpf: String,
    results_sender: watch::Sender<Vec<Vec<Value>>>,
) {
    let mut local_cmd = tokio::process::Command::new("bpftrace");
    local_cmd.arg("-f");
    local_cmd.arg("json");
    local_cmd.arg("-e");
    local_cmd.arg(bpf);
    local_cmd.stdout(Stdio::piped());
    local_cmd.stderr(Stdio::piped());

    let mut handle = local_cmd.spawn().unwrap();
    let stdout = handle.stdout.take().unwrap();
    let stdout_reader = BufReader::new(stdout);

    let stderr = handle.stderr.as_mut().unwrap();
    let stderr_reader = BufReader::new(stderr);

    let mut lines = stdout_reader.lines();
    let mut errors = stderr_reader.lines();

    let mut rows = [].to_vec();

    loop {
        tokio::select! {
        error = errors.next_line() => match error {
            Ok(Some(line)) => {
                    rows = [[Value::String(line)].to_vec()].to_vec();
                    results_sender.send(rows.clone()).unwrap();
                    break;
            }
            Ok(None) => break, // End of stream
            Err(e) => {
                println!("Error reading line: {:?}", e);
                break;
            }
        },
        line = lines.next_line() => {
            match line {
                Ok(Some(line)) => {
                    //parse json
                    if line.is_empty() {
                        continue;
                    }
                    let v: serde_json::Value = serde_json::from_str(&line).unwrap();
                    if v["type"] == "attached_probes" {
                        continue;
                    }
                    if v["type"] == "map" {
                        //for now, this indicates the end of the query
                        rows.push([Value::String("DONE".to_string())].to_vec());
                        results_sender.send(rows).unwrap();
                        break;
                    }
                    //convert array of key value pairs to dict

                    let mut d = HashMap::new();
                    let mut id = 0;

                    for kv in v["data"].as_array().unwrap() {
                        let k = &kv[0];
                        let v = &kv[1];
                        match k {
                            serde_json::Value::Number(_) => {
                                let i = k.as_u64().unwrap();
                                let h = headers[i as usize].clone();
                                d.insert(h.to_string(), v.clone());
                            }
                            serde_json::Value::String(_) => {
                                id = v.as_u64().unwrap();
                            }
                            _ => {
                                println!("unexpected key type");
                            }
                        }
                    }

                    let mut row = Vec::new();
                    //put the id in
                    row.push(Value::Number(serde_json::Number::from(id)));
                    for header in headers.clone() {
                        let value = &d[header.as_str()];
                        row.push(value.clone());
                    }
                    rows.push(row);
                    let result = results_sender.send(rows.clone());
                    match result {
                        Ok(_) => {}
                        Err(e) => {
                            println!("Error sending results: {:#?}", e);
                            break;
                        }
                    }
                }
                Ok(None) => {
                    results_sender.send([[Value::String("Done".to_string())].to_vec()].to_vec()).unwrap();
                }, // End of stream
                Err(e) => {
                    println!("Error reading line: {:?}", e);
                    break;
                }
            }
        }
        }
    }
}

pub async fn execute_bpf_remotely(
    hostname: String,
    headers: Vec<String>,
    bpf: String,
    results_sender: watch::Sender<Vec<Vec<Value>>>,
) {
    let mut h = hostname.clone();
    let mut s = SessionBuilder::default();
    if hostname == "bpftrace_machine" {
        h = std::env::var("BPFTRACE_MACHINE").unwrap();
        let user = "root".to_string();
        h = format!("{}@{}", user, h);
        s.keyfile("/app/bpftrace_machine");
    }
    s.known_hosts_check(KnownHosts::Accept);

    let session = s.connect(h).await.unwrap();

    let mut remote_cmd = session.command("bpftrace");
    remote_cmd.arg("-f");
    remote_cmd.arg("json");
    remote_cmd.arg("-e");
    remote_cmd.arg(bpf);
    remote_cmd.stdout(Stdio::piped());
    remote_cmd.stderr(Stdio::piped());

    let mut handle = remote_cmd.spawn().await.unwrap();
    let stdout = handle.stdout().take().unwrap();
    let stdout_reader = BufReader::new(stdout);

    let stderr = handle.stderr().as_mut().unwrap();
    let stderr_reader = BufReader::new(stderr);

    let mut lines = stdout_reader.lines();
    let mut errors = stderr_reader.lines();

    let mut rows = [].to_vec();

    loop {
        tokio::select! {
        error = errors.next_line() => match error {
            Ok(Some(line)) => {
                    rows = [[Value::String(line)].to_vec()].to_vec();
                    results_sender.send(rows.clone()).unwrap();
                    break;
            }
            Ok(None) => break, // End of stream
            Err(e) => {
                println!("Error reading line: {:?}", e);
                break;
            }
        },
        line = lines.next_line() => {
            match line {
                Ok(Some(line)) => {
                    //parse json
                    if line.is_empty() {
                        continue;
                    }
                    let v: serde_json::Value = serde_json::from_str(&line).unwrap();
                    if v["type"] == "attached_probes" {
                        continue;
                    }
                    if v["type"] == "map" {
                        //for now, this indicates the end of the query
                        rows.push([Value::String("DONE".to_string())].to_vec());
                        results_sender.send(rows).unwrap();
                        break;
                    }
                    //convert array of key value pairs to dict

                    let mut d = HashMap::new();
                    let mut id = 0;

                    for kv in v["data"].as_array().unwrap() {
                        let k = &kv[0];
                        let v = &kv[1];
                        match k {
                            serde_json::Value::Number(_) => {
                                let i = k.as_u64().unwrap();
                                let h = headers[i as usize].clone();
                                d.insert(h.to_string(), v.clone());
                            }
                            serde_json::Value::String(_) => {
                                id = v.as_u64().unwrap();
                            }
                            _ => {
                                println!("unexpected key type");
                            }
                        }
                    }

                    let mut row = Vec::new();
                    //put the id in
                    row.push(Value::Number(serde_json::Number::from(id)));
                    for header in headers.clone() {
                        let value = &d[header.as_str()];
                        row.push(value.clone());
                    }
                    rows.push(row);
                    let result = results_sender.send(rows.clone());
                    match result {
                        Ok(_) => {}
                        Err(e) => {
                            println!("Error sending results: {:#?}", e);
                            break;
                        }
                    }
                }
                Ok(None) => {
                    results_sender.send([[Value::String("Done".to_string())].to_vec()].to_vec()).unwrap();
                }, // End of stream
                Err(e) => {
                    println!("Error reading line: {:?}", e);
                    break;
                }
            }
        }
        }
    }
}

pub async fn execute_bpf(
    hostname: String,
    headers: Vec<String>,
    bpf: String,
    results_sender: watch::Sender<Vec<Vec<Value>>>,
) {
    if hostname == "localhost" {
        execute_bpf_locally(headers, bpf, results_sender).await;
    } else {
        execute_bpf_remotely(hostname, headers, bpf, results_sender).await;
    }
}
