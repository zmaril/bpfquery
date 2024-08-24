import perspective from "https://cdn.jsdelivr.net/npm/@finos/perspective@3.0.0/dist/cdn/perspective.js";
import * as monaco from "https://cdn.jsdelivr.net/npm/monaco-editor@0.51.0/+esm";

const uri = "ws://" + location.host + "/bpfquery";
const ws = new WebSocket(uri);

editor = monaco.editor.create(document.getElementById("editor"), {
  value: `select
      str(args.path -> dentry -> d_name.name)
  from
      kprobe.vfs_open;
  `,
  language: "sql",
  overviewRulerLanes: 0, // This turns off the overview ruler
  minimap: { enabled: false }, // This turns off the minimap if desired
});

hljs.highlightAll();

function sendSql() {
  ws.send(editor.getValue());
}

ws.onopen = function () {
  sendSql();
};

ws.onmessage = function (msg) {
  let bpfv = document.getElementById("bpftrace-viewer");
  let d = JSON.parse(msg.data);

  if (d.msg_type == "bpftrace_output") {
    bpfv.innerText = d.data.output;
    bpfv.classList.add("bg-gray-200");
    bpfv.classList.remove("bg-red-200");
  } else if (d.msg_type = "bpftrace_error") {
    bpfv.innerText = d.data.error_message;
    bpfv.classList.add("bg-red-200");
    bpfv.classList.remove("bg-gray-200");
  }
  else {
    alert("Unknown message type: " + d.msg_type);
  }

  //unset data-highlighted on this eleent
  //document.getElementById("bpftrace-viewer").removeAttribute("data-highlighted");
  //hljs.highlightAll();
};

editor.getModel().onDidChangeContent((event) => {
  sendSql();
});

// ws.onmessage = function (msg) {
//   message(msg.data);
// };

// ws.onclose = function () {
//   chat.getElementsByTagName("em")[0].innerText = "Disconnected!";
// };

// send.onclick = function () {
//   const msg = text.value;
//   ws.send(msg);
//   text.value = "";

//   message("<You>: " + msg);
// };

const worker = await perspective.worker();
const resp = await fetch(
  "https://cdn.jsdelivr.net/npm/superstore-arrow/superstore.lz4.arrow"
);
const arrow = await resp.arrayBuffer();
const viewer = document.getElementsByTagName("perspective-viewer")[0];
const table = worker.table(arrow);
viewer.load(table);
viewer.restore({ settings: true, plugin_config: { edit_mode: "EDIT" } });
