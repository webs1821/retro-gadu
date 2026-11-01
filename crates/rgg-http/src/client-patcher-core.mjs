// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
// Independent JS implementation of our Python recipe. Format facts: THIRD_PARTY.toml.
// The supplied plan pins a whole-file SHA256. No user executable is executed or uploaded.
export class PatchError extends Error {
    constructor(code) { super(code); this.code = code; }
}
function requireValue(value, code = 'invalid_plan') {
    if (!value) throw new PatchError(code);
}
export function validHostname(value) {
    return typeof value === 'string' && value.length >= 1 && value.length <= 16
        && /^[A-Za-z0-9]+(?:[.-][A-Za-z0-9]+)*$/.test(value);
}
export async function sha256(bytes, provider = globalThis.crypto) {
    requireValue(provider?.subtle, 'crypto_unavailable');
    const digest = new Uint8Array(await provider.subtle.digest('SHA-256', bytes));
    return Array.from(digest, byte => byte.toString(16).padStart(2, '0')).join('');
}
function bounded(value, minimum, maximum) {
    return Number.isSafeInteger(value) && value >= minimum && value <= maximum;
}
function unhex(value) {
    requireValue(typeof value === 'string' && value.length <= 131072 && /^(?:[0-9a-f]{2})+$/.test(value));
    return Uint8Array.from(value.match(/../g), pair => Number.parseInt(pair, 16));
}
function decode(bytes, width) {
    requireValue(bytes.length % width === 0);
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    let value = '';
    for (let start = 0; start < bytes.length; start += 2048 * width) {
        const units = [];
        for (let i = start; i < Math.min(bytes.length, start + 2048 * width); i += width) {
            units.push(width === 1 ? view.getUint8(i) : view.getUint16(i, true));
        }
        value += String.fromCharCode(...units);
    }
    return value;
}
function encode(text, width) {
    const result = new Uint8Array(text.length * width);
    const view = new DataView(result.buffer);
    for (let i = 0; i < text.length; i++) {
        const unit = text.charCodeAt(i);
        if (width === 1) { requireValue(unit <= 255); view.setUint8(i, unit); }
        else view.setUint16(i * 2, unit, true);
    }
    return result;
}
function rewrite(text, domains, host) {
    const pattern = new RegExp(domains.map(domain => domain.replaceAll('.', '\\.')).join('|'), 'g');
    return text.replace(pattern, () => host);
}
function table(before, domains, host) {
    const view = new DataView(before.buffer, before.byteOffset, before.byteLength);
    const parts = [];
    let offset = 0, total = 0;
    for (let i = 0; i < 16; i++) {
        requireValue(offset + 2 <= before.length);
        const units = view.getUint16(offset, true); offset += 2;
        requireValue(offset + units * 2 <= before.length);
        const text = rewrite(decode(before.subarray(offset, offset + units * 2), 2), domains, host);
        offset += units * 2;
        requireValue(text.length <= 65535);
        const encoded = encode(text, 2), part = new Uint8Array(encoded.length + 2);
        new DataView(part.buffer).setUint16(0, text.length, true); part.set(encoded, 2);
        parts.push(part); total += part.length;
    }
    const tail = before.subarray(offset);
    requireValue(total + tail.length <= before.length, 'replacement_too_long');
    const result = new Uint8Array(before.length);
    offset = 0;
    for (const part of parts) { result.set(part, offset); offset += part.length; }
    result.set(tail, offset);
    return result;
}
function validatePlan(plan) {
    requireValue(plan?.schema === 1 && bounded(plan.client_size, 256, 2 * 1024 * 1024));
    requireValue(typeof plan.client_sha256 === 'string' && /^[0-9a-f]{64}$/.test(plan.client_sha256));
    requireValue(Array.isArray(plan.domains) && plan.domains.length > 0 && plan.domains.length <= 16);
    requireValue(plan.domains.every(name => typeof name === 'string' && name.length <= 64 && /^[a-z0-9.-]+$/.test(name)));
    requireValue(new Set(plan.domains).size === plan.domains.length);
    requireValue(Array.isArray(plan.edits) && plan.edits.length > 0 && plan.edits.length <= 128);
    requireValue(plan.edits.every(edit => edit && typeof edit === 'object'));
    const edits = plan.edits.map(edit => ({ offset: edit.offset, length: edit.length, kind: edit.kind,
        width: edit.width, before: edit.before, after: edit.after })).sort((a, b) => a.offset - b.offset);
    let end = 0;
    for (const edit of edits) {
        requireValue(bounded(edit.offset, end, plan.client_size) && bounded(edit.length, 1, 65536));
        requireValue(edit.offset + edit.length <= plan.client_size);
        requireValue(['fixed', 'cstring', 'rt_string'].includes(edit.kind));
        if (edit.kind === 'cstring') requireValue([1, 2].includes(edit.width) && edit.length % edit.width === 0);
        end = edit.offset + edit.length;
    }
    return { size: plan.client_size, hash: plan.client_sha256, domains: [...plan.domains], edits };
}
export async function patchClient(input, host, plan, provider = globalThis.crypto) {
    requireValue(validHostname(host), 'invalid_host');
    const checked = validatePlan(plan);
    requireValue(input instanceof Uint8Array && input.byteLength === checked.size, 'unsupported_client');
    // Copy before awaiting a digest: callers cannot change a verified buffer mid-operation.
    const source = new Uint8Array(input);
    requireValue(await sha256(source, provider) === checked.hash, 'unsupported_client');
    const output = new Uint8Array(source);
    for (const edit of checked.edits) {
        const before = source.subarray(edit.offset, edit.offset + edit.length);
        let after;
        if (edit.kind === 'fixed') {
            const expected = unhex(edit.before); after = unhex(edit.after);
            requireValue(expected.length === before.length && after.length === before.length);
            requireValue(expected.every((value, i) => value === before[i]), 'patch_precondition');
        } else if (edit.kind === 'rt_string') after = table(before, checked.domains, host);
        else {
            const encoded = encode(rewrite(decode(before, edit.width), checked.domains, host), edit.width);
            requireValue(encoded.length <= before.length, 'replacement_too_long');
            after = new Uint8Array(before.length); after.set(encoded);
        }
        output.set(after, edit.offset);
    }
    return { bytes: output, sha256: await sha256(output, provider) };
}
