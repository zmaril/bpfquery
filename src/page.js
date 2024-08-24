import perspective from "https://cdn.jsdelivr.net/npm/@finos/perspective@3.0.0/dist/cdn/perspective.js";
import * as monaco from "https://cdn.jsdelivr.net/npm/monaco-editor@0.51.0/+esm";

const uri = "ws://" + location.host + "/bpfquery";
const ws = new WebSocket(uri);

// const chat = document.getElementById("chat");
// const text = document.getElementById("text");

// function message(data) {
//   const line = document.createElement("p");
//   line.innerText = data;
//   chat.appendChild(line);
// }

// ws.onopen = function () {
//   chat.innerHTML = "<p><em>Connected!</em></p>";
// };

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

console.log("here");
monaco.editor.create(document.getElementById("editor"), {
  value: `select
    str(args.path -> dentry -> d_name.name)
from
    kprobe.vfs_open;

`,
  language: "sql",
  overviewRulerLanes: 0, // This turns off the overview ruler
  minimap: { enabled: false }, // This turns off the minimap if desired
});

const worker = await perspective.worker();
const resp = await fetch(
  "https://cdn.jsdelivr.net/npm/superstore-arrow/superstore.lz4.arrow"
);
const arrow = await resp.arrayBuffer();
const viewer = document.getElementsByTagName("perspective-viewer")[0];
const table = worker.table(arrow);
viewer.load(table);
viewer.restore({ settings: true, plugin_config: { edit_mode: "EDIT" } });

hljs.highlightAll();