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
  wordWrap: "on",
});

hljs.highlightAll();

function sendSql() {
  try {
    ws.send(editor.getValue());
  }
  catch (e) {
    // ignore for now 
  }
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
  table = await worker.table([{ Loading: "Data" }], {});
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

let rows = [];

ws.onmessage = async function (msg) {
  //check if editor has focus
  let focused = editor.hasTextFocus();
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
    rows = [];
    await reload_perspective();
  } else if (d.msg_type == "bpftrace_error") {
    bpfv.innerText = d.error_message;
    bpfv.classList.add("bg-red-200");
    bpfv.classList.remove("bg-gray-200");
  } else if (d.msg_type == "bpftrace_results") {
    if (d.results.length == 0) {
      return;
    }
    // transform results into a perspective table
    let data = {};
    for (let i = 0; i < headers.length; i++) {
      data[headers[i]] = d.results[i];
    }
    rows.push(data);
    if (first_load) {
      table = await worker.table(rows, { index: "id" });
      elem.load(table);
      first_load = false;
    }
    if (showing_example) {
      elem.restore(examples[example_selector.value].config);
      showing_example = false;
    }
  } else {
    alert("Unknown message type: " + d.msg_type);
  }

  if (focused) {
    editor.focus();
  }
};

editor.getModel().onDidChangeContent((event) => {
  sendSql();
});

monaco.languages.registerCompletionItemProvider("sql", {
  provideCompletionItems: (model, position) => {
    // const suggestions = keywords.map(keyword => ({
    //     label: keyword,
    //     kind: monaco.languages.CompletionItemKind.Keyword,
    //     insertText: keyword
    // }));
    return { suggestions: [] };
  },
});

let examples = {
  start: {
    sql: `select comm, probe from tracepoint.syscalls.sys_enter_STAR;`,
    config: {
      version: "3.0.1",
      plugin: "Datagrid",
      plugin_config: { columns: {}, edit_mode: "READ_ONLY", scroll_lock: true },
      columns_config: {},
      settings: false,
      theme: "Pro Light",
      title: null,
      group_by: ["comm", "probe"],
      split_by: [],
      columns: ["id"],
      filter: [],
      sort: [],
      expressions: {},
      aggregates: { id: "count" },
    },
  },
  start: {
    sql: `select
      str(args.path -> dentry -> d_name.name) as filename
  from
      kprobe.vfs_open;
  `,
    config: {
      version: "3.0.1",
      plugin: "Datagrid",
      plugin_config: { columns: {}, edit_mode: "READ_ONLY", scroll_lock: true },
      columns_config: {},
      settings: false,
      theme: "Pro Light",
      title: null,
      group_by: [],
      split_by: [],
      columns: [],
      filter: [],
      sort: [],
      expressions: {},
      aggregates: {},
    },
  },
  systemcalls: {
    sql: `select comm, probe from tracepoint.syscalls.sys_enter_STAR;`,
    config: {
      version: "3.0.1",
      plugin: "Datagrid",
      plugin_config: { columns: {}, edit_mode: "READ_ONLY", scroll_lock: true },
      columns_config: {},
      settings: false,
      theme: "Pro Light",
      title: null,
      group_by: ["comm", "probe"],
      split_by: [],
      columns: ["id"],
      filter: [],
      sort: [],
      expressions: {},
      aggregates: { id: "count" },
    },
  },
  kprobe: {
    sql: `--I ran ctags across the linux kernel source code, pulled out all the signatures and then reference the signature when compiling the query, so you don't have to do a bunch of casts in the query.
select
  pid, comm, str(args.path -> dentry -> d_name.name) as filename
from kprobe.vfs_open;`,
    config: {
      version: "3.0.1",
      plugin: "Datagrid",
      plugin_config: { columns: {}, edit_mode: "READ_ONLY", scroll_lock: true },
      columns_config: {},
      settings: false,
      theme: "Pro Light",
      title: null,
      group_by: [],
      split_by: [],
      columns: ["id", "pid", "comm", "filename"],
      filter: [],
      sort: [],
      expressions: {},
      aggregates: {},
    },
  },
};

var example_selector = document.getElementById("example_selector");

var showing_example = false;

function show_example(name) {
  editor.setValue(examples[name].sql);
  sendSql();
  showing_example = true;
  // update the query param
  window.history.replaceState(
    {},
    "",
    window.location.pathname + "?showing=" + name
  );
}
//whenever somebody selects an example, look it up in examples and then set the editor value to that
example_selector.addEventListener("change", function () {
  show_example(example_selector.value);
});

//look at query params to see if we should load an example
const urlParams = new URLSearchParams(window.location.search);
const example = urlParams.get("showing");
if (example) {
  //set the example selector to the example
  example_selector.value = example;
  show_example(example);
}

let old_length = 0;
let i = setInterval(async () => {
  if (rows.length > 0 && rows.length != old_length) {
    console.log("updating table");
    console.log(rows);
    await table.replace(rows);
    elem.load(table);
    old_length = rows.length;
  }
}, 1000);