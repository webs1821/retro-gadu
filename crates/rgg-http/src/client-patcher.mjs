// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
import { patchClient, validHostname } from '/assets/client-patcher-core.js';

const form = document.getElementById('patch-form');
const fileField = document.getElementById('client-file');
const hostField = document.getElementById('server-host');
const button = document.getElementById('prepare-client');
const status = document.getElementById('patch-status');
const download = document.getElementById('patched-download');
const result = document.getElementById('patch-result');
const checksum = document.getElementById('patch-sha');
let plan, busy = false, generation = 0, url;
function clearDownload() {
    generation++;
    if (url) URL.revokeObjectURL(url);
    url = undefined;
    download.removeAttribute('href'); download.hidden = true;
    result.hidden = true; checksum.textContent = ''; status.textContent = '';
}
for (const field of [fileField, hostField]) field.addEventListener('input', clearDownload);
window.addEventListener('pagehide', clearDownload);
const messages = {
    invalid_host: 'Podaj nazwę lub IPv4: od 1 do 16 znaków, bez protokołu, portu i ścieżki.',
    unsupported_client: 'To nie jest oryginalny gg.exe z GG 6.1.0.158. Wybierz niezmieniony plik z tej wersji.',
    crypto_unavailable: 'Ta przeglądarka nie udostępnia bezpiecznej kontroli pliku. Otwórz stronę lokalnie w aktualnej przeglądarce.',
};
form.addEventListener('submit', async event => {
    event.preventDefault();
    if (busy || !plan) return;
    clearDownload();
    const current = generation, file = fileField.files[0], host = hostField.value;
    if (!file) { status.textContent = 'Wybierz oryginalny gg.exe.'; return; }
    if (!validHostname(host)) { status.textContent = messages.invalid_host; return; }
    if (file.size !== plan.client_size) { status.textContent = messages.unsupported_client; return; }
    busy = true; button.disabled = fileField.disabled = hostField.disabled = true;
    status.textContent = 'Sprawdzanie i przygotowywanie kopii…';
    try {
        const output = await patchClient(new Uint8Array(await file.arrayBuffer()), host, plan);
        if (current !== generation) return;
        url = URL.createObjectURL(new Blob([output.bytes], {type: 'application/octet-stream'}));
        download.href = url; download.hidden = false;
        checksum.textContent = output.sha256; result.hidden = false;
        status.textContent = 'Gotowe. Pobierz nową kopię dla serwera '+host+'.';
    } catch (error) {
        if (current === generation) status.textContent = messages[error.code] || 'Nie udało się bezpiecznie przygotować pliku. Oryginał nie został zmieniony.';
    } finally {
        busy = false; button.disabled = fileField.disabled = hostField.disabled = false;
    }
});
try {
    if (!globalThis.crypto?.subtle) throw new Error('crypto_unavailable');
    const response = await fetch('/assets/client-patch-plan.json', {credentials: 'omit', cache: 'no-store'});
    if (!response.ok) throw new Error('plan_unavailable');
    const text = await response.text();
    if (text.length > 16384) throw new Error('plan_unavailable');
    plan = JSON.parse(text);
    button.disabled = false; status.textContent = 'Wybierz plik, aby rozpocząć.';
} catch (error) {
    status.textContent = messages[error.message] || 'Nie udało się wczytać narzędzia. Odśwież stronę i spróbuj ponownie.';
}
