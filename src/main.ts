import './style.css';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { openUrl } from '@tauri-apps/plugin-opener';

interface AppError { message: string; detail: string }
const fields = [
  ['toolkit', 'HackingToolkit3DS 폴더', 'dir'], ['decryptor', 'Decryptor 폴더', 'dir'],
  ['base', '본편 원본 (북미·UNDUB·DECRYPTED)', '3ds'], ['patch1', '본편 1차 패치', 'zip'], ['patch2', '본편 2차 패치', 'zip'],
  ['dlc', 'DLC 원본 (UNDECRYPTED)', 'cia'], ['dlcPatch', 'DLC 패치', 'zip'],
  ['work', '작업 폴더', 'dir'], ['output', '출력 폴더', 'dir'],
] as const;
export const app = document.querySelector<HTMLElement>('#app')!;
app.innerHTML = `<header><span>Windows · SMT IV FINAL</span><h1>진여신전생 4F 한글 패처</h1><p>원본을 보존하며 한글 패치와 결과 파일을 만듭니다.</p></header>
<section><h2>1. 외부 도구 준비</h2><p>도구를 직접 다운로드하고 ZIP을 해제한 폴더를 선택하세요.</p><p><button id="toolLink" class="link">포크 HackingToolkit3DS 받기 ↗</button> <button id="decryptLink" class="link">Batch CIA 3DS Decryptor 받기 ↗</button></p><div id="tools"></div></section>
<section><h2>2. 원본과 패치 선택</h2><p>본편은 지정된 북미 UNDUB 원본이 필요합니다. 패치는 ZIP만 지원합니다.</p><div id="inputs"></div><label class="toggle"><input id="useDlc" type="checkbox"> DLC도 패치하기</label><div id="dlcInputs" hidden></div></section>
<section><h2>3. 결과 설정</h2><div id="folders"></div><label>본편 형식 <select id="format"><option value="cia">CIA</option><option value="threeds">3DS</option></select></label><p>DLC 결과는 CIA로 저장됩니다. 작업·출력 폴더에 충분한 여유 공간을 확보하세요.</p></section>
<section><button id="start" disabled>패치하기</button><button id="cancel" hidden>취소</button><p id="status" role="status" aria-live="polite">필수 경로와 원본 해시를 확인하세요.</p><div id="progress"></div><pre id="result" tabindex="0"></pre><button id="copy" hidden>상세 오류 복사</button><button id="openOutput" hidden>출력 폴더 열기</button></section>`;
export const values: Record<string, string> = {};
let stored: Record<string, string> = {};
try { stored = JSON.parse(localStorage.getItem('4f-tools') || '{}'); } catch { /* recover corrupt preferences */ }
export let busy = false;
export function setBusy(value: boolean) { busy = value; app.querySelectorAll<HTMLInputElement | HTMLButtonElement | HTMLSelectElement>('input, button, select').forEach(el => { if (!['cancel', 'copy', 'openOutput'].includes(el.id)) el.disabled = value; }); if (!value) void refresh(); }
const hashes: Record<string, string> = {};
let revision = 0;
export function showError(error: unknown) {
  document.querySelector('#status')!.textContent = (error as AppError).message || String(error);
  document.querySelector('#result')!.textContent = JSON.stringify(error, null, 2);
  document.querySelector<HTMLElement>('#copy')!.hidden = false;
}
export function request() { return { ...values, dlc: useDlc() ? values.dlc : null, dlcPatch: useDlc() ? values.dlcPatch : null, format: document.querySelector<HTMLSelectElement>('#format')!.value }; }
function useDlc() { return document.querySelector<HTMLInputElement>('#useDlc')!.checked; }
async function refresh() {
  const version = ++revision;
  const start = document.querySelector<HTMLButtonElement>('#start')!; start.disabled = true;
  if (busy || !values.base || hashes.base !== values.base || (useDlc() && (!values.dlc || hashes.dlc !== values.dlc))) return;
  try { await invoke('check_inputs', { request: request() }); if (version === revision && !busy) { start.disabled = false; document.querySelector('#status')!.textContent = '준비되었습니다. 패치하기를 누르세요.'; } }
  catch (e) { if (version === revision) document.querySelector('#status')!.textContent = (e as AppError).message; }
}
for (const [key, label, kind] of fields) {
  const row = document.createElement('div'); row.className = 'field';
  const title = document.createElement('label'); title.htmlFor = key; title.textContent = label;
  const line = document.createElement('div'); line.className = 'path';
  const input = document.createElement('input'); input.id = key; input.readOnly = true; input.placeholder = '선택되지 않음'; input.value = values[key] = stored[key] || '';
  const button = document.createElement('button'); button.textContent = '찾아보기'; button.setAttribute('aria-label', `${label} 선택`);
  const status = document.createElement('small'); status.setAttribute('aria-live', 'polite');
  line.append(input, button); row.append(title, line, status);
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
      } else { status.textContent = '선택됨'; }
      await refresh();
    } catch (e) { showError(e); }
  };
}
document.querySelector<HTMLInputElement>('#useDlc')!.onchange = () => { document.querySelector<HTMLElement>('#dlcInputs')!.hidden = !useDlc(); void refresh(); };
document.querySelector('#toolLink')!.addEventListener('click', () => { void openUrl('https://github.com/b4r0k0v/HackingToolkit3DS/releases/download/v9/HackingToolkit3DSv9.zip').catch(showError); });
document.querySelector('#decryptLink')!.addEventListener('click', () => { void openUrl('https://gbatemp.net/download/batch-cia-3ds-decryptor.35098/download?version=35152').catch(showError); });
document.querySelector('#copy')!.addEventListener('click', () => { void navigator.clipboard.writeText(document.querySelector('#result')!.textContent || '').catch(showError); });
