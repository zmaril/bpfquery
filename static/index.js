import perspective from "https://cdn.jsdelivr.net/npm/@finos/perspective@3.0.0/dist/cdn/perspective.js";
import * as monaco from "https://cdn.jsdelivr.net/npm/monaco-editor@0.51.0/+esm";

let protocol = "wss:";
if (location.protocol === "http:") {
  protocol = "ws:";
}


const uri = protocol + location.host + "/bpfquery";
const ws = new WebSocket(uri);

editor = monaco.editor.create(document.getElementById("editor"), {
  value: `select
      str(args.path -> dentry -> d_name.name) as filename
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

var headers = [];

var worker; 
var table; 
var elem;


let first_load = false;

async function reload_perspective() {
  first_load = true;
  worker = await perspective.worker();
    elem = document.getElementsByTagName("perspective-viewer")[0];
    table = await worker.table([{"Loading": "Data"}], {});
    await elem.load(table);
    elem.restore({
      plugin: "Datagrid",
      plugin_config: {
          editable: false,
          scroll_lock: true,
      },
      settings: false,
      theme: "Pro Light",
      filter: [],
  });
}

ws.onmessage = async function (msg) {
  let bpfv = document.getElementById("bpftrace-viewer");
  let d = JSON.parse(msg.data);
  if (worker === undefined) {
    // first message, start up 
    await reload_perspective();
  }

  if (d.msg_type == "bpftrace_output") {
    bpfv.innerText = d.output;
    headers = ["id"];
    headers = headers.concat(d.headers);
    bpfv.classList.add("bg-gray-200");
    bpfv.classList.remove("bg-red-200");
    await reload_perspective();

  } else if (d.msg_type == "bpftrace_error") {
    console.log("huh")
    bpfv.innerText = d.error_message;
    bpfv.classList.add("bg-red-200");
    bpfv.classList.remove("bg-gray-200");
  }
  else if (d.msg_type == "bpftrace_results") {
    if (d.results.length == 0 ) {
      return;
    }
    console.log(d)
    // transform results into a perspective table
    let data = d.results.map((row) => {
      let obj = {};
      for (let i = 0; i < headers.length; i++) {
        obj[headers[i]] = row[i];
      }
      return obj;
    });
    if (first_load) {
      table = await worker.table(data, {index: "id"});
      elem.load(table);
      first_load = false;
    }
    else {
      table.replace(data);
    }
  }
  else {
    alert("Unknown message type: " + d.msg_type);
  }
};

editor.getModel().onDidChangeContent((event) => {
  sendSql();
});

