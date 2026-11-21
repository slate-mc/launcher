import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { lstat, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { execFileSync } from "node:child_process";
import { pathToFileURL } from "node:url";
import { parseArgs } from "node:util";

const platforms = ["windows-x86_64", "linux-x86_64", "darwin-aarch64", "darwin-x86_64"];
const immutableCache = "public, max-age=31536000, immutable";
const channelCache = "no-cache, max-age=0, must-revalidate";

async function sha256(path) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest("hex");
}

export async function prepareRelease({ source, channel, expectedVersion, publicBase }) {
  if (!["stable", "beta"].includes(channel)) throw new Error("Unknown release channel.");
  const base = new URL(publicBase);
  if (base.protocol !== "https:" || base.username || base.password || base.search || base.hash) {
    throw new Error("The public download origin must be a plain HTTPS URL.");
  }
  const manifest = JSON.parse(await readFile(join(source, "latest.json"), "utf8"));
  const version = manifest.version?.replace(/^v/, "");
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(version)) {
    throw new Error("The release version must be SemVer.");
  }
  if (version !== expectedVersion) throw new Error("The manifest and requested release version differ.");
  if (channel === "stable" && version.split("+")[0].includes("-")) {
    throw new Error("A prerelease cannot be published to stable.");
  }
  const prefix = `releases/${version}`;
  const objects = [];
  const assets = new Map();
  for (const name of (await readdir(source)).sort()) {
    if (name === "latest.json") continue;
    if (!/^[\w.+() -]+$/.test(name)) throw new Error(`Unsafe release asset name: ${name}`);
    const path = join(source, name);
    const stat = await lstat(path);
    if (!stat.isFile() || stat.isSymbolicLink()) throw new Error(`Release assets must be regular files: ${name}`);
    const object = {
      key: `${prefix}/${name}`, path, name, size: stat.size,
      sha256: await sha256(path), contentType: "application/octet-stream", cacheControl: immutableCache,
    };
    objects.push(object);
    assets.set(name, object);
  }
  const urlFor = (key) => new URL(key.split("/").map(encodeURIComponent).join("/"), `${base.href.replace(/\/$/, "")}/`).href;
  for (const platform of platforms) {
    if (!manifest.platforms?.[platform]) throw new Error(`The release is missing ${platform}.`);
  }
  for (const [platform, entry] of Object.entries(manifest.platforms)) {
    const assetUrl = new URL(entry.url);
    const name = decodeURIComponent(assetUrl.pathname.split("/").at(-1));
    if (name !== basename(name) || !assets.has(name)) throw new Error(`Missing updater artifact for ${platform}.`);
    const signature = await readFile(join(source, `${name}.sig`), "utf8");
    if (!signature.trim() || signature.trim() !== entry.signature?.trim()) {
      throw new Error(`The signature does not match the updater manifest for ${platform}.`);
    }
    entry.url = urlFor(assets.get(name).key);
  }
  const serialized = `${JSON.stringify(manifest, null, 2)}\n`;
  const downloads = {
    version: manifest.version, pub_date: manifest.pub_date, notes: manifest.notes,
    files: objects.filter(({ name }) => !name.endsWith(".sig")).map(({ name, key, size, sha256 }) => ({
      name, url: urlFor(key), size, sha256,
    })),
  };
  const jsonObject = (key, body, cacheControl) => ({
    key, body, contentType: "application/json", cacheControl,
    size: Buffer.byteLength(body), sha256: createHash("sha256").update(body).digest("hex"),
  });
  objects.push(jsonObject(`${prefix}/latest.json`, serialized, immutableCache));
  const pointers = [
    jsonObject(`channels/${channel}/downloads.json`, `${JSON.stringify(downloads, null, 2)}\n`, channelCache),
    jsonObject(`channels/${channel}/latest.json`, serialized, channelCache),
  ];
  return { version, channel, objects, pointers, urlFor };
}

// Every immutable asset is checked before writing; channel pointers are written last.
export async function publishRelease(release, storage) {
  const existing = new Map();
  for (const object of release.objects) {
    const current = await storage.head(object.key);
    if (current && (current.sha256 !== object.sha256 || current.size !== object.size)) {
      throw new Error(`Refusing to replace an immutable release object: ${object.key}`);
    }
    existing.set(object.key, current);
  }
  for (const object of release.objects) {
    if (!existing.get(object.key)) await storage.put(object);
    await storage.verify(object);
  }
  for (const object of release.pointers) {
    await storage.put(object);
    await storage.verify(object);
  }
}

async function main() {
  const { values } = parseArgs({ options: {
    source: { type: "string" }, channel: { type: "string", default: "stable" },
    version: { type: "string" }, "dry-run": { type: "boolean", default: false },
  } });
  if (!values.source || !values.version) throw new Error("Pass --source and --version.");
  const publicBase = process.env.SLATE_RELEASE_PUBLIC_URL || "https://downloads.slatemc.app";
  const release = await prepareRelease({
    source: resolve(values.source), channel: values.channel, expectedVersion: values.version, publicBase,
  });
  if (values["dry-run"]) {
    console.log(JSON.stringify({ version: release.version, objects: [...release.objects, ...release.pointers].map(({ key, size }) => ({ key, size })) }, null, 2));
    return;
  }
  const bucket = process.env.SLATE_R2_BUCKET || "slate-releases";
  const endpoint = new URL(process.env.SLATE_R2_ENDPOINT);
  if (endpoint.protocol !== "https:" || !endpoint.hostname.endsWith(".r2.cloudflarestorage.com") || endpoint.username || endpoint.password || endpoint.pathname !== "/" || endpoint.search || endpoint.hash) {
    throw new Error("SLATE_R2_ENDPOINT must be the HTTPS R2 S3 API endpoint.");
  }
  if (!process.env.AWS_ACCESS_KEY_ID || !process.env.AWS_SECRET_ACCESS_KEY) throw new Error("R2 publishing credentials are required.");
  const temporary = await mkdtemp(join(tmpdir(), "slate-r2-publish-"));
  const aws = (args) => execFileSync("aws", ["--endpoint-url", endpoint.href, "--region", "auto", "--no-cli-pager", "s3api", ...args], {
    encoding: "utf8", stdio: ["ignore", "pipe", "pipe"],
    env: { ...process.env, AWS_REQUEST_CHECKSUM_CALCULATION: "WHEN_REQUIRED", AWS_RESPONSE_CHECKSUM_VALIDATION: "WHEN_REQUIRED" },
  });
  const head = async (key) => {
    try {
      const object = JSON.parse(aws(["head-object", "--bucket", bucket, "--key", key]));
      return { sha256: object.Metadata?.sha256, size: object.ContentLength };
    } catch (error) {
      if (/\(404\)|NotFound|Not Found/.test(String(error.stderr))) return undefined;
      throw error;
    }
  };
  try {
    await publishRelease(release, {
      head,
      async put(object) {
        let path = object.path;
        if (!path) {
          path = join(temporary, `${object.sha256}.json`);
          await writeFile(path, object.body);
        }
        aws(["put-object", "--bucket", bucket, "--key", object.key, "--body", path,
          "--content-type", object.contentType, "--cache-control", object.cacheControl,
          "--metadata", JSON.stringify({ sha256: object.sha256 })]);
        console.log(`Uploaded ${object.key}`);
      },
      async verify(object) {
        const stored = await head(object.key);
        if (stored?.sha256 !== object.sha256 || stored.size !== object.size) throw new Error(`Upload verification failed: ${object.key}`);
        const response = await fetch(release.urlFor(object.key), { method: "HEAD", signal: AbortSignal.timeout(30_000) });
        if (!response.ok || Number(response.headers.get("content-length")) !== object.size) throw new Error(`Public download verification failed: ${object.key}`);
      },
    });
    console.log(`Published ${release.version} to ${publicBase}/channels/${release.channel}/latest.json`);
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main().catch((error) => { console.error(error.message); process.exitCode = 1; });
}
