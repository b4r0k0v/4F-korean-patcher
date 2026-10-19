import './style.css';
import { invoke } from '@tauri-apps/api/core';
import { open, confirm } from '@tauri-apps/plugin-dialog';
import { openUrl } from '@tauri-apps/plugin-opener';
import { listen } from '@tauri-apps/api/event';

interface AppError { message: string; detail: string }
const fields = [
  ['toolkit', 'HackingToolkit3DS 폴더', 'dir'], ['decryptor', 'Decryptor 폴더', 'dir'],
  ['base', '본편 원본 (북미·UNDUB·DECRYPTED)', '3ds'], ['patch1', '본편 1차 패치', 'zip'], ['patch2', '본편 2차 패치', 'zip'],
  ['dlc', 'DLC 원본 (UNDECRYPTED)', 'cia'], ['dlcPatch', 'DLC 패치', 'zip'],
  ['work', '작업 폴더', 'dir'], ['output', '출력 폴더', 'dir'],
] as const;
export const app = document.querySelector<HTMLElement>('#app')!;
app.innerHTML = `<header><span>Windows · SMT IV FINAL</span><h1>진여신전생 4F 한글 패처</h1><p>원본을 보존하며 한글 패치와 결과 파일을 만듭니다.</p></header>
<nav role="tablist" aria-label="패치 준비 단계">
<button id="tab-tools" class="step-tab" role="tab" aria-controls="panel-tools" aria-selected="true" data-step="tools">1. 도구 준비</button>
<button id="tab-files" class="step-tab" role="tab" aria-controls="panel-files" aria-selected="false" tabindex="-1" data-step="files">2. 파일 선택</button>
<button id="tab-output" class="step-tab" role="tab" aria-controls="panel-output" aria-selected="false" tabindex="-1" data-step="output">3. 결과 설정</button>
</nav><div id="setup">
<section id="panel-tools" class="setup-panel" role="tabpanel" aria-labelledby="tab-tools"><h2>외부 도구 준비</h2><p>도구를 직접 다운로드하고 ZIP을 해제한 폴더를 선택하세요.</p><p><button id="toolLink" class="link">포크 HackingToolkit3DS 받기 ↗</button> <button id="decryptLink" class="link">Batch CIA 3DS Decryptor 받기 ↗</button></p><div id="tools"></div><div class="step-actions"><button data-step="files">다음: 파일 선택 →</button></div></section>
<section id="panel-files" class="setup-panel" role="tabpanel" aria-labelledby="tab-files" hidden><h2>원본과 패치 선택</h2><p>본편은 지정된 북미 UNDUB 원본이 필요합니다. 패치는 ZIP만 지원합니다.</p><div id="inputs"></div><label class="toggle"><input id="useDlc" type="checkbox"> DLC도 패치하기</label><div id="dlcInputs" hidden></div><div class="step-actions"><button data-step="output">다음: 결과 설정 →</button></div></section>
<section id="panel-output" class="setup-panel" role="tabpanel" aria-labelledby="tab-output" hidden><h2>결과 설정</h2><div id="folders"></div><label>본편 형식 <select id="format"><option value="cia">CIA</option><option value="threeds">3DS</option></select></label><p>DLC 결과는 CIA로 저장됩니다. 작업·출력 폴더에 충분한 여유 공간을 확보하세요.</p><p>준비를 마쳤으면 아래의 <strong>패치하기</strong>를 누르세요.</p></section>
</div><section id="run-panel" aria-label="실행 및 결과"><div class="run-actions"><button id="start" disabled>패치하기</button><button id="cancel" hidden>취소</button><button id="copy" hidden>상세 오류 복사</button><button id="openOutput" hidden>출력 폴더 열기</button></div><p id="status" role="status" aria-live="polite">필수 경로와 원본 해시를 확인하세요.</p><div id="progress"></div><pre id="result" tabindex="0"></pre></section>`;
const tabs = Array.from(app.querySelectorAll<HTMLButtonElement>('.step-tab'));
function selectStep(step: string) {
  for (const tab of tabs) {
    const selected = tab.dataset.step === step;
    tab.setAttribute('aria-selected', String(selected)); tab.tabIndex = selected ? 0 : -1;
    document.querySelector<HTMLElement>(`#${tab.getAttribute('aria-controls')}`)!.hidden = !selected;
  }
  document.querySelector('#setup')!.scrollTop = 0;
}
app.querySelectorAll<HTMLButtonElement>('[data-step]').forEach(button => {
  button.onclick = () => { selectStep(button.dataset.step!); if (!button.matches('.step-tab')) tabs.find(tab => tab.dataset.step === button.dataset.step)?.focus(); };
});
tabs.forEach((tab, index) => tab.onkeydown = (event: KeyboardEvent) => {
  const next = event.key === 'ArrowRight' ? (index + 1) % tabs.length : event.key === 'ArrowLeft' ? (index + tabs.length - 1) % tabs.length : event.key === 'Home' ? 0 : event.key === 'End' ? tabs.length - 1 : -1;
  if (next < 0 || tabs[next].disabled) return;
  event.preventDefault(); selectStep(tabs[next].dataset.step!); tabs[next].focus();
});
export const values: Record<string, string> = {};
let stored: Record<string, string> = {};
try { stored = JSON.parse(localStorage.getItem('4f-tools') || '{}'); } catch { /* recover corrupt preferences */ }
export let busy = false;
export function setBusy(value: boolean) { busy = value; app.querySelectorAll<HTMLInputElement | HTMLButtonElement | HTMLSelectElement>('input, button, select').forEach(el => { if (!['cancel', 'copy', 'openOutput'].includes(el.id)) el.disabled = value; }); if (!value) void refresh(false); }
const hashes: Record<string, string> = {};
let revision = 0;
export function showError(error: unknown) {
  document.querySelector('#status')!.textContent = (error as AppError).message || String(error);
  document.querySelector('#result')!.textContent = JSON.stringify(error, null, 2);
  document.querySelector<HTMLElement>('#copy')!.hidden = false;
}
export function request() { return { ...values, dlc: useDlc() ? values.dlc : null, dlcPatch: useDlc() ? values.dlcPatch : null, format: document.querySelector<HTMLSelectElement>('#format')!.value }; }
function useDlc() { return document.querySelector<HTMLInputElement>('#useDlc')!.checked; }
async function refresh(updateStatus = true) {
  const version = ++revision;
  const start = document.querySelector<HTMLButtonElement>('#start')!; start.disabled = true;
  if (busy || !values.base || hashes.base !== values.base || (useDlc() && (!values.dlc || hashes.dlc !== values.dlc))) return;
  try { await invoke('check_inputs', { request: request() }); if (version === revision && !busy) { start.disabled = false; if (updateStatus) document.querySelector('#status')!.textContent = '준비되었습니다. 패치하기를 누르세요.'; } }
  catch (e) { if (version === revision && updateStatus) document.querySelector('#status')!.textContent = (e as AppError).message; }
}
for (const [key, label, kind] of fields) {
  const row = document.createElement('div'); row.className = 'field';
  const title = document.createElement('label'); title.htmlFor = key; title.textContent = label;
  const line = document.createElement('div'); line.className = 'path';
  const input = document.createElement('input'); input.id = key; input.readOnly = true; input.placeholder = '선택되지 않음'; input.value = values[key] = stored[key] || '';
  const button = document.createElement('button'); button.textContent = '찾아보기'; button.setAttribute('aria-label', `${label} 선택`);
  const status = document.createElement('small'); status.setAttribute('aria-live', 'polite');
  line.append(input, button); row.append(title, line, status);
  const checkPath = async (path: string) => {
    try { await invoke('check_field', { path, kind: ['toolkit', 'decryptor'].includes(key) ? key : key === 'patch1' ? 'first' : kind === 'zip' ? 'patch' : kind }); if (values[key] === path) status.textContent = '사용 가능'; }
    catch (e) { if (values[key] === path) status.textContent = (e as AppError).message; }
  };
  if (input.value) void checkPath(input.value);
  document.querySelector(kind === 'dir' ? (['toolkit', 'decryptor'].includes(key) ? '#tools' : '#folders') : (key.startsWith('dlc') ? '#dlcInputs' : '#inputs'))!.append(row);
  button.onclick = async () => {
    try {
      const path = await open({ directory: kind === 'dir', multiple: false, filters: kind === 'dir' ? undefined : [{ name: kind.toUpperCase(), extensions: [kind] }] });
      if (typeof path !== 'string' || busy) return;
      values[key] = input.value = path; delete hashes[key]; ++revision;
      document.querySelector<HTMLButtonElement>('#start')!.disabled = true;
      if (['toolkit', 'decryptor'].includes(key)) { stored[key] = path; localStorage.setItem('4f-tools', JSON.stringify(stored)); }
      if (key === 'base' || key === 'dlc') {
        status.textContent = 'SHA-256 검사 중…';
        try { await invoke('check_hash', { path, dlc: key === 'dlc' }); if (values[key] === path) { hashes[key] = path; status.textContent = 'SHA-256 일치'; } }
        catch (e) { if (values[key] === path) status.textContent = (e as AppError).message; }
      } else { await checkPath(path); }
      await refresh();
    } catch (e) { showError(e); }
  };
}
document.querySelector<HTMLInputElement>('#useDlc')!.onchange = () => { document.querySelector<HTMLElement>('#dlcInputs')!.hidden = !useDlc(); void refresh(); };
document.querySelector('#toolLink')!.addEventListener('click', () => { void openUrl('https://github.com/b4r0k0v/HackingToolkit3DS/releases/download/v9/HackingToolkit3DSv9.zip').catch(showError); });
document.querySelector('#decryptLink')!.addEventListener('click', () => { void openUrl('https://gbatemp.net/download/batch-cia-3ds-decryptor.35098/download?version=35152').catch(showError); });
document.querySelector('#copy')!.addEventListener('click', () => { void navigator.clipboard.writeText(document.querySelector('#result')!.textContent || '').catch(showError); });

const start = document.querySelector<HTMLButtonElement>('#start')!;
const cancel = document.querySelector<HTMLButtonElement>('#cancel')!;
const outputButton = document.querySelector<HTMLButtonElement>('#openOutput')!;
const status = document.querySelector('#status')!;
const result = document.querySelector('#result')!;
const progress = document.querySelector('#progress')!;
let pendingClose = false;
await listen<{ stage: string; completed: number; total: number }>('patch-progress', ({ payload: p }) => {
  status.textContent = `${p.stage} · ${p.completed}/${p.total}단계 완료`;
  const bar = document.createElement('progress'); bar.max = p.total; bar.value = p.completed;
  bar.setAttribute('aria-label', '완료한 패치 단계'); progress.replaceChildren(bar);
});
await listen('patch-close-requested', async () => {
  if (pendingClose) return;
  pendingClose = true;
  try { if (await confirm('패치를 취소하고 종료할까요? 도구 종료를 기다린 뒤 앱을 닫습니다. 작업 폴더는 보존됩니다.', { title: '패치 실행 중', kind: 'warning' })) { await invoke('cancel_patch', { close: true }); status.textContent = '도구를 종료하고 앱을 닫는 중…'; } }
  catch (e) { showError(e); }
  finally { pendingClose = false; }
});
start.onclick = async () => {
  if (busy) return;
  const inputs = request(); setBusy(true); cancel.hidden = false; cancel.disabled = false;
  outputButton.hidden = true; document.querySelector<HTMLElement>('#copy')!.hidden = true;
  result.textContent = ''; progress.replaceChildren(); status.textContent = '실행 전 원본 재검사 중…';
  try {
    const outcome = await invoke<{ files: string[]; warning?: string }>('start_patch', { request: inputs });
    status.textContent = outcome.warning || '패치 파일 생성이 완료되었습니다. 게임 내 동작은 별도로 확인하세요.';
    const bar = progress.querySelector('progress'); if (bar) bar.value = bar.max;
    result.textContent = outcome.files.join('\n'); outputButton.hidden = false;
  } catch (e) { showError(e); }
  finally { cancel.hidden = true; setBusy(false); }
};
cancel.onclick = async () => {
  cancel.disabled = true;
  try { await invoke('cancel_patch', { close: false }); status.textContent = '취소 중… 현재 도구가 종료될 때까지 기다려 주세요.'; }
  catch (e) { showError(e); cancel.disabled = false; }
};
outputButton.onclick = () => { void invoke('open_output', { path: values.output }).catch(showError); };
