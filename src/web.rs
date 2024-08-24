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

/// Our global unique user id counter.
static NEXT_USER_ID: AtomicUsize = AtomicUsize::new(1);

/// Our state of currently connected users.
///
/// - Key is their id
/// - Value is a sender of `warp::ws::Message`
type Users = Arc<RwLock<HashMap<usize, mpsc::UnboundedSender<Message>>>>;
type Tasks = Arc<RwLock<HashMap<usize, mpsc::UnboundedSender<Message>>>>;

pub async fn start_server(hostname: String) {
    pretty_env_logger::init();

    // Keep track of all connected users, key is usize, value
    // is a websocket sender.
    let users = Users::default();

    // Keep track of all the spawn tasks
    let tasks = Tasks::default();
    // Turn our "state" into a new Filter...
    let users = warp::any().map(move || users.clone());
    let tasks = warp::any().map(move || tasks.clone());

    // GET /chat -> websocket upgrade
    let editor = warp::path("bpfquery")
        // The `ws()` filter will prepare Websocket handshake...
        .and(warp::ws())
        .and(users)
        .and(tasks)
        .map(move |ws: warp::ws::Ws, users, tasks| {
            let h = hostname.clone();
            // This will call our function if the handshake succeeds.
            ws.on_upgrade(move |socket| user_connected(h, socket, users, tasks))
        });

    // GET / -> index html
    let index = warp::path::end().map(|| {
        let s = std::fs::read_to_string("src/page.html").unwrap();
        warp::reply::html(s)
    });

    let js = warp::path("page.js").map(|| {
        let s = std::fs::read_to_string("src/page.js").unwrap();
        warp::reply::with_header(s, "content-type", "text/javascript")
    });

    let routes = index.or(editor).or(js);

    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
    let metrics = Handle::current().metrics();
    dbg!(metrics.num_workers());
    dbg!(metrics.num_alive_tasks());
}

async fn user_connected(hostname: String, ws: WebSocket, users: Users, tasks: Tasks) {
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
            user_ws_tx
                .send(message)
                .unwrap_or_else(|e| {
                    eprintln!("websocket send error: {}", e);
                })
                .await;
        }
    });

    // Save the sender in our list of connected users.
    users.write().await.insert(my_id, tx);

    // Return a `Future` that is basically a state machine managing
    // this specific user's connection.

    let sql = "select
      str(args.path -> dentry -> d_name.name)
  from
      kprobe.vfs_open;"
        .to_string();
    let ast = parse_bpfquery_sql(&sql).unwrap();
    let (mut output, mut headers) = compile_ast_to_bpftrace(ast).unwrap();
    let (mut results_sender, mut results_reciver) = tokio::sync::watch::channel([].to_vec());
    let h = hostname.clone();
    let hds = headers.clone();
    let ot = output.clone();
    let mut t = tokio::task::spawn(async {
        execute_bpf(h, hds, ot, results_sender).await;
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
               break;
           }
           _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {
              dbg!("here!!");
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
                          ResponseData::BpftraceOutput(BpftraceOutputMsg {
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
                          (results_sender, results_reciver) = tokio::sync::watch::channel([].to_vec());
                          t.abort();
                          let h = hostname.clone();
                          let hds = headers.clone();
                          let ot = output.clone();
                          t = tokio::task::spawn(async {
                              execute_bpf(h, hds, ot, results_sender).await;
                          });
                      }
                  }
              }
          }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {
                let data = results_reciver.borrow().clone();
                if data.len() == 1 && data[0].len() == 1 {
                    println!("{}", data[0][0]);
                    break;
                }
                else if !data.is_empty() && data[data.len()-1].len() == 1 && data[data.len()-1][0] == "DONE" {
                    for d in data {
                        println!("{}", d[1..].iter().map(|x| x.to_string()).collect::<Vec<String>>().

                            join(", "));
                    }
                    break;
                }
                println!("{:?}", data);
            }
        }
    }
    // user_ws_rx stream will keep processing as long as the user stays
    // connected. Once they disconnect, then...
    user_disconnected(my_id, &users).await;
}

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
#[serde(tag = "type", content = "data")]
enum ResponseData {
    BpftraceOutput(BpftraceOutputMsg),
    BpftraceError(BpftraceErrorMsg),
}

#[derive(Serialize, Clone)]
struct ResponseMessage {
    #[serde(flatten)]
    data: ResponseData,
    msg_type: String,
}

async fn user_message(my_id: usize, msg: Message, users: &Users) -> Option<ResponseMessage> {
    // Skip any non-Text messages...
    let msg = if let Ok(s) = msg.to_str() {
        s
    } else {
        return None;
    };

    let new_msg = format!("<User#{}>: {}", my_id, msg);
    dbg!("new_msg: {}", &new_msg);

    let result = parse_bpfquery_sql(msg);
    let response = match result {
        Ok(ast) => {
            let (output, headers) = compile_ast_to_bpftrace(ast).unwrap();
            // convert to json
            ResponseMessage {
                data: ResponseData::BpftraceOutput(BpftraceOutputMsg { output, headers }),
                msg_type: "bpftrace_output".to_string(),
            }
        }
        Err(e) => ResponseMessage {
            data: ResponseData::BpftraceError(BpftraceErrorMsg {
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
