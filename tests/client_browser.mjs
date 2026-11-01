// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { webcrypto } from 'node:crypto';
import { patchClient, sha256, validHostname, PatchError } from '../crates/rgg-http/src/client-patcher-core.mjs';

const vectors = JSON.parse(await readFile(process.argv[2], 'utf8'));
for (const vector of vectors) {
    const input = Uint8Array.from(Buffer.from(vector.input, 'base64'));
    const original = input.slice();
    const expected = Buffer.from(vector.expected, 'base64');
    const plan = vector.plan;
    const result = await patchClient(input, vector.host, plan, webcrypto);
    assert.deepEqual(Buffer.from(result.bytes), expected);
    assert.deepEqual(input, original);
    assert.equal(result.sha256, await sha256(expected, webcrypto));
    for (const host of ['', 'a'.repeat(17), 'bad/name', 'a..b', 'a--b', '-host', 'żółć']) {
        assert.equal(validHostname(host), false);
        await assert.rejects(patchClient(input, host, plan, webcrypto), error => error.code === 'invalid_host');
    }
    const changed = input.slice(); changed[0] ^= 1;
    await assert.rejects(patchClient(changed, vector.host, plan, webcrypto), error => error.code === 'unsupported_client');
    await assert.rejects(patchClient(new Uint8Array(1), vector.host, plan, webcrypto), error => error.code === 'unsupported_client');
    for (const mutate of [
        value => value.edits.push({...value.edits[0]}),
        value => value.edits[0].offset = -1,
        value => value.edits[0].length = value.client_size,
        value => value.edits[0].offset = Number.MAX_SAFE_INTEGER,
        value => value.edits[0].kind = 'unknown',
        value => value.edits[0] = null,
        value => value.domains.push(value.domains[0]),
        value => value.edits.find(edit => edit.kind === 'fixed').before = '00',
    ]) {
        const copy = structuredClone(plan); mutate(copy);
        await assert.rejects(patchClient(input, vector.host, copy, webcrypto), PatchError);
    }
    const badPrefix = input.slice(), malformed = structuredClone(plan);
    const tableOffset = malformed.edits.find(edit => edit.kind === 'rt_string').offset;
    badPrefix[tableOffset] = badPrefix[tableOffset + 1] = 255;
    malformed.client_sha256 = await sha256(badPrefix, webcrypto);
    await assert.rejects(patchClient(badPrefix, vector.host, malformed, webcrypto), PatchError);
    const mismatched = structuredClone(plan), fixed = mismatched.edits.find(edit => edit.kind === 'fixed');
    fixed.before = '00'.repeat(fixed.length);
    await assert.rejects(patchClient(input, vector.host, mismatched, webcrypto), error => error.code === 'patch_precondition');
    assert.deepEqual(input, original);
    // Both the file and plan are snapshotted before asynchronous digest work.
    const racedInput = input.slice(), racedPlan = structuredClone(plan);
    let first = true;
    const provider = {subtle: {digest: async (...args) => {
        if (first) { first = false; racedInput.fill(0); racedPlan.domains.length = 0; racedPlan.edits[0].offset = 0; racedPlan.client_sha256 = '0'.repeat(64); }
        return webcrypto.subtle.digest(...args);
    }}};
    assert.deepEqual(Buffer.from((await patchClient(racedInput, vector.host, racedPlan, provider)).bytes), expected);
}
console.log('PASS: browser core matches independent Python output, bounds/hash/host/preconditions and async snapshots; '+vectors.length+' vectors');
