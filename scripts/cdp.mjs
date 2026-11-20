// Outil d'essai : pilote la vraie interface d'une instance lancée avec
//   WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<port>
// Usage :
//   node scripts/cdp.mjs <port> <page> eval "<javascript>"
//   node scripts/cdp.mjs <port> <page> shot <fichier.png>
// <page> = main (fenêtre principale) ou notif (notification)
const [port, page, action, arg] = process.argv.slice(2);
const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
const isNotif = (u) => u.includes("notif.html");
const target = targets.find((t) => t.type === "page" && (page === "notif" ? isNotif(t.url) : !isNotif(t.url)));
if (!target) {
  console.error("page introuvable :", targets.map((t) => t.url));
  process.exit(1);
}
const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((ok, ko) => { ws.onopen = ok; ws.onerror = ko; });
let n = 0;
const call = (method, params = {}) => new Promise((ok, ko) => {
  const id = ++n;
  const on = (e) => {
    const m = JSON.parse(e.data);
    if (m.id !== id) return;
    ws.removeEventListener("message", on);
    m.error ? ko(new Error(m.error.message)) : ok(m.result);
  };
  ws.addEventListener("message", on);
  ws.send(JSON.stringify({ id, method, params }));
});
if (action === "eval") {
  const r = await call("Runtime.evaluate", { expression: arg, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails) {
    console.error("erreur :", r.exceptionDetails.exception?.description || r.exceptionDetails.text);
    process.exitCode = 1;
  } else {
    console.log(typeof r.result.value === "string" ? r.result.value : JSON.stringify(r.result.value));
  }
} else if (action === "shot") {
  const { writeFileSync } = await import("node:fs");
  const r = await call("Page.captureScreenshot", { format: "png" });
  writeFileSync(arg, Buffer.from(r.data, "base64"));
  console.log("capture :", arg);
}
ws.close();
