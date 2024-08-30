// #![deny(warnings)]
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use crate::bpftrace_compiler::compile_ast_to_bpftrace;
use crate::executor::execute_bpf;
use crate::parser::parse_bpfquery_sql;

use futures_util::{SinkExt, StreamExt, TryFutureExt};
use serde::Serialize;
use tokio::runtime::Handle;
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::ws::{Message, WebSocket};
use warp::Filter;

#[derive(Serialize, Clone)]
struct BpftraceOutputMsg {
    output: String,
    headers: Vec<String>,
}

#[derive(Serialize, Clone)]
struct BpftraceErrorMsg {
    error_message: String,
}

#[derive(Serialize, Clone)]
struct BpftraceResults {
    results: Vec<serde_json::Value>,
}

#[derive(Serialize, Clone)]
#[serde(untagged)]
enum ResponseData {
    Output(BpftraceOutputMsg),
    Error(BpftraceErrorMsg),
    Results(BpftraceResults),
}

#[derive(Serialize, Clone)]
struct ResponseMessage {
    #[serde(flatten)]
    data: ResponseData,
    msg_type: String,
}

/// Our global unique user id counter.
static NEXT_USER_ID: AtomicUsize = AtomicUsize::new(1);

/// Our state of currently connected users.
///
/// - Key is their id
/// - Value is a sender of `warp::ws::Message`
type Users = Arc<RwLock<HashMap<usize, mpsc::UnboundedSender<Message>>>>;

pub async fn start_server(hostname: String, demo: bool) {
    pretty_env_logger::init();

    // Keep track of all connected users, key is usize, value
    // is a websocket sender.
    let users = Users::default();

    // Turn our "state" into a new Filter...
    let users = warp::any().map(move || users.clone());

    // GET /chat -> websocket upgrade
    let editor = warp::path("bpfquery")
        // The `ws()` filter will prepare Websocket handshake...
        .and(warp::ws())
        .and(users)
        .map(move |ws: warp::ws::Ws, users| {
            let h = hostname.clone();
            // This will call our function if the handshake succeeds.
            ws.on_upgrade(move |socket| user_connected(h, socket, users, demo))
        });

    let static_files = warp::fs::dir("static");

    let routes = static_files.or(editor);

    warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;
    let metrics = Handle::current().metrics();
    dbg!(metrics.num_workers());
    dbg!(metrics.num_alive_tasks());
}

async fn user_connected(hostname: String, ws: WebSocket, users: Users, demo: bool) {
    // Use a counter to assign a new unique ID for this user.
    let my_id = NEXT_USER_ID.fetch_add(1, Ordering::Relaxed);

    eprintln!("new chat user: {}", my_id);

    // Split the socket into a sender and receive of messages.
    let (mut user_ws_tx, mut user_ws_rx) = ws.split();

    // Use an unbounded channel to handle buffering and flushing of messages
    // to the websocket...
    let (tx, rx) = mpsc::unbounded_channel();
    let mut rx = UnboundedReceiverStream::new(rx);

    let tt = tokio::task::spawn(async move {
        while let Some(message) = rx.next().await {
            let mut broken = false;
            user_ws_tx
                .send(message)
                .unwrap_or_else(|e| {
                    eprintln!("websocket send error: {}", e);
                    broken = true;
                })
                .await;
            if broken {
                break;
            }
        }
    });

    // Save the sender in our list of connected users.
    users.write().await.insert(my_id, tx);

    // Return a `Future` that is basically a state machine managing
    // this specific user's connection.

    let sql = "select
      str(args.path -> dentry -> d_name.name) as filename
  from
      kprobe.vfs_open;"
        .to_string();
    let ast = parse_bpfquery_sql(&sql).unwrap();
    let (mut output, mut headers) = compile_ast_to_bpftrace(ast).unwrap();
    let (mut results_sender, mut results_reciver) = tokio::sync::broadcast::channel(10000);
    let h = hostname.clone();
    let hds = headers.clone();
    let ot = output.clone();
    let d = demo.clone();
    let mut t = tokio::task::spawn(async move {
        execute_bpf(h, hds, ot, results_sender, d).await;
    });

    // Every time the user sends a message, broadcast it to
    // all other users...
    loop {
        tokio::task::yield_now().await;
        tokio::select! {
           _ = tokio::signal::ctrl_c() => {
               t.abort();
               //TODO, ctrl-c does not work here at all
               std::process::exit(1);
           }
          result = user_ws_rx.next() => {
              if let Some(result) = result {
                  let msg = match result {
                      Ok(msg) => msg,
                      Err(e) => {
                          eprintln!("websocket error(uid={}): {}", my_id, e);
                          break;
                      }
                  };
                  let response = user_message(my_id, msg, &users).await;

                  if let Some(ResponseMessage {
                      data:
                          ResponseData::Output(BpftraceOutputMsg {
                              output: new_output,
                              headers: new_headers,
                          }),
                      msg_type: _,
                  }) = response.clone()
                  {
                      //only restart task if the output or headers have changed
                      if new_output != output || new_headers != headers {
                          output = new_output;
                          headers = new_headers;
                          (results_sender, results_reciver) = tokio::sync::broadcast::channel(10000);
                          t.abort();
                          let h = hostname.clone();
                          let hds = headers.clone();
                          let ot = output.clone();
                          let d = demo.clone();
                          t = tokio::task::spawn(async move {
                              execute_bpf(h, hds, ot, results_sender, d).await;
                          });
                      }
                  }
              }
          }
          data = results_reciver.recv() => {
              if let Ok(data) = data {
                  if !data.is_empty() && data[0] == "DONE" {
                      break;
                  }
                  if let Some(tx) = users.read().await.get(&my_id) {
                      let response = ResponseMessage {
                          data: ResponseData::Results(BpftraceResults { results: data }),
                          msg_type: "bpftrace_results".to_string(),
                      };
                      let response_string = serde_json::to_string(&response).unwrap();
                      if let Err(_disconnected) = tx.send(Message::text(response_string.clone())) {
                          println!("error sending message to user: {}", my_id);
                          break;
                      }
                  }
              }
            }
        }
    }
    // user_ws_rx stream will keep processing as long as the user stays
    // connected. Once they disconnect, then...
    t.abort();
    tt.abort();
    user_disconnected(my_id, &users).await;
}

async fn user_message(my_id: usize, msg: Message, users: &Users) -> Option<ResponseMessage> {
    // Skip any non-Text messages...
    let msg = if let Ok(s) = msg.to_str() {
        s
    } else {
        return None;
    };

    let result = parse_bpfquery_sql(msg);
    let response = match result {
        Ok(ast) => {
            let result2 = compile_ast_to_bpftrace(ast);
            match result2 {
                Ok((output, headers)) => ResponseMessage {
                    data: ResponseData::Output(BpftraceOutputMsg { output, headers }),
                    msg_type: "bpftrace_output".to_string(),
                },
                Err(e) => ResponseMessage {
                    data: ResponseData::Error(BpftraceErrorMsg {
                        error_message: e.to_string(),
                    }),
                    msg_type: "bpftrace_error".to_string(),
                },
            }
        }
        Err(e) => ResponseMessage {
            data: ResponseData::Error(BpftraceErrorMsg {
                error_message: e.to_string(),
            }),
            msg_type: "bpftrace_error".to_string(),
        },
    };

    //make it only reload on changes
    let response_string = serde_json::to_string(&response).unwrap();

    // New message for this user, send it to only this users
    if let Some(tx) = users.read().await.get(&my_id) {
        if let Err(_disconnected) = tx.send(Message::text(response_string.clone())) {
            // The tx is disconnected, our `user_disconnected` code
            // should be happening in another task, nothing more to
            // do here.

            //TODO abort the task
        }
    }
    Some(response)
}

async fn user_disconnected(my_id: usize, users: &Users) {
    eprintln!("good bye user: {}", my_id);

    // Stream closed up, so remove from the user list
    users.write().await.remove(&my_id);
    dbg!("done removing user");
}
