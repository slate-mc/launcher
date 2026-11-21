import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { prepareRelease, publishRelease } from "./publish-launcher-r2.mjs";

async function fixture(t) {
  const source = await mkdtemp(join(tmpdir(), "slate-r2-test-"));
  t.after(() => rm(source, { recursive: true, force: true }));
  const manifest = { version: "0.1.0", platforms: {} };
  for (const platform of ["windows-x86_64", "linux-x86_64", "darwin-aarch64", "darwin-x86_64"]) {
    const name = `slate-${platform}.bin`;
    await writeFile(join(source, name), `signed installer for ${platform}`);
    await writeFile(join(source, `${name}.sig`), `signature-${platform}\n`);
    manifest.platforms[platform] = { url: `https://github.com/slate-mc/launcher/releases/download/slate-v0.1.0/${name}`, signature: `signature-${platform}` };
  }
  await writeFile(join(source, "latest.json"), JSON.stringify(manifest));
  return { source, channel: "stable", expectedVersion: "0.1.0", publicBase: "https://downloads.slatemc.app" };
}

test("rewrites all signed platform URLs to immutable R2 downloads", async (t) => {
  const input = await fixture(t);
  const release = await prepareRelease(input);
  const manifest = JSON.parse(release.pointers.at(-1).body);
  for (const entry of Object.values(manifest.platforms)) {
    assert.match(entry.url, /^https:\/\/downloads\.slatemc\.app\/releases\/0\.1\.0\//);
    assert.ok(entry.signature.startsWith("signature-"));
  }
  assert.equal(release.objects.length, 9);
  assert.equal(release.pointers.at(-1).key, "channels/stable/latest.json");
  assert.match(release.pointers.at(-1).cacheControl, /no-cache/);
});

test("rejects incomplete builds and mismatched signatures before uploading", async (t) => {
  const input = await fixture(t);
  await writeFile(join(input.source, "slate-windows-x86_64.bin.sig"), "different signature");
  await assert.rejects(prepareRelease(input), /signature does not match/);
  await rm(join(input.source, "slate-windows-x86_64.bin"));
  await assert.rejects(prepareRelease(input), /Missing updater artifact/);
});

test("rejects wrong versions and prereleases on stable", async (t) => {
  const input = await fixture(t);
  await assert.rejects(prepareRelease({ ...input, expectedVersion: "0.2.0" }), /versions? differ/);
  const manifest = JSON.parse(await readFile(join(input.source, "latest.json"), "utf8"));
  manifest.version = "0.2.0-beta.1";
  await writeFile(join(input.source, "latest.json"), JSON.stringify(manifest));
  await assert.rejects(prepareRelease({ ...input, expectedVersion: manifest.version }), /prerelease/);
});

test("never advances the update channel when any artifact fails verification", async (t) => {
  const release = await prepareRelease(await fixture(t));
  const writes = [];
  await assert.rejects(publishRelease(release, {
    async head() { return undefined; },
    async put(object) { writes.push(object.key); },
    async verify() { throw new Error("verification failed"); },
  }), /verification failed/);
  assert.ok(writes.length > 0);
  assert.ok(writes.every((key) => key.startsWith("releases/")));
});

test("refuses to replace a published immutable release", async (t) => {
  const release = await prepareRelease(await fixture(t));
  await assert.rejects(publishRelease(release, {
    async head() { return { sha256: "different", size: 1 }; },
    async put() { assert.fail("Must not overwrite the release"); },
    async verify() { assert.fail("Must fail before any write"); },
  }), /immutable release object/);
});

test("verifies every release object before publishing channel pointers", async (t) => {
  const release = await prepareRelease(await fixture(t));
  const operations = [];
  await publishRelease(release, {
    async head() { return undefined; },
    async put(object) { operations.push(`put:${object.key}`); },
    async verify(object) { operations.push(`verify:${object.key}`); },
  });
  const firstPointer = operations.findIndex((entry) => entry.startsWith("put:channels/"));
  assert.equal(firstPointer, release.objects.length * 2);
  assert.equal(operations.at(-2), "put:channels/stable/latest.json");
});
