// Run against a locally launched WebView2 with its debugging port explicitly enabled.
import { writeFile, mkdir } from 'node:fs/promises';
import { resolve } from 'node:path';
const port = process.argv[2] || '9227';
const pages = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
const page = pages.find(p => p.type === 'page');
if (!page) throw new Error('WebView2 page not found');
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((ok, bad) => { ws.onopen = ok; ws.onerror = bad; });
let id = 0;
const pending = new Map();
ws.onmessage = ({ data }) => { const p = JSON.parse(data); if (p.id && pending.has(p.id)) { const [ok, bad] = pending.get(p.id); pending.delete(p.id); p.error ? bad(p.error) : ok(p.result); } };
function send(method, params = {}) { return new Promise((ok, bad) => { const key = ++id; pending.set(key, [ok, bad]); ws.send(JSON.stringify({ id: key, method, params })); }); }
async function evaluate(expression) { const result = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true }); if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails)); return result.result.value; }
try {
  const initial = await evaluate(`({ title: document.title, inputs: document.querySelectorAll('.path input').length, disabled: document.querySelector('#start').disabled, dlcHidden: document.querySelector('#dlcInputs').hidden })`);
  if (initial.inputs !== 9 || !initial.disabled || !initial.dlcHidden) throw new Error(JSON.stringify(initial));
  if (await evaluate(`document.querySelector('#dlcInputs').hidden`)) await evaluate(`document.querySelector('#useDlc').click()`);
  if (await evaluate(`document.querySelector('#dlcInputs').hidden`)) throw new Error('DLC toggle failed');
  const toolkit = resolve('dev-temp/HackingToolkit3DSv9');
  await evaluate(`window.__TAURI_INTERNALS__.invoke('check_field',{path:${JSON.stringify(toolkit)},kind:'toolkit'})`);
  const error = await evaluate(`window.__TAURI_INTERNALS__.invoke('check_field',{path:'missing.zip',kind:'patch'}).then(()=>null,e=>e.message)`);
  if (!error || !error.includes('.zip')) throw new Error('IPC validation error missing');
  const screenshot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: true });
  await mkdir('dev-temp/gui', { recursive: true });
  await writeFile('dev-temp/gui/screenshot.png', Buffer.from(screenshot.data, 'base64'));
  console.log(JSON.stringify({ ...initial, toolStatus: 'available', ipcError: error, screenshot: 'dev-temp/gui/screenshot.png' }));
} finally { ws.close(); }
