import { ModelFile } from './stt.models';

/**
 * Model files in the browser: Cache Storage (keyed by the pinned URL), so every model is
 * downloaded once. Each download is checked against size and SHA-256 from the catalogue before it
 * is cached. Without Cache Storage (private windows …) files are downloaded every time.
 */
const CACHE = 'talk-typer-models-v1';

export class ModelFileError extends Error {
  constructor(
    readonly code: 'download_failed' | 'checksum_mismatch',
    message: string,
  ) {
    super(message);
  }
}

async function openCache(): Promise<Cache | null> {
  try {
    return typeof caches === 'undefined' ? null : await caches.open(CACHE);
  } catch {
    return null;
  }
}

export async function sha256Hex(data: Uint8Array<ArrayBuffer>): Promise<string> {
  const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', data));
  return Array.from(digest, (b) => b.toString(16).padStart(2, '0')).join('');
}

/** Returns the file's bytes; `onBytes(n)` reports bytes of this file received so far. */
export async function loadModelFile(
  file: ModelFile,
  onBytes: (n: number) => void,
): Promise<Uint8Array<ArrayBuffer>> {
  const cache = await openCache();
  const cached = await cache?.match(file.url).catch(() => undefined);
  if (cached) {
    const buf = new Uint8Array(await cached.arrayBuffer());
    if (buf.length === file.size) {
      onBytes(buf.length);
      return buf;
    }
    await cache?.delete(file.url).catch(() => false);
  }

  let resp: Response;
  try {
    resp = await fetch(file.url);
  } catch (e) {
    throw new ModelFileError('download_failed', String(e));
  }
  if (!resp.ok || !resp.body) {
    throw new ModelFileError('download_failed', `${file.url}: HTTP ${resp.status}`);
  }
  const buf = new Uint8Array(file.size);
  let n = 0;
  const reader = resp.body.getReader();
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    if (n + value.length > file.size) {
      throw new ModelFileError('checksum_mismatch', `${file.url} is larger than expected`);
    }
    buf.set(value, n);
    n += value.length;
    onBytes(n);
  }
  if (n !== file.size || (await sha256Hex(buf)) !== file.sha256) {
    throw new ModelFileError('checksum_mismatch', file.url);
  }
  await cache
    ?.put(file.url, new Response(buf, { headers: { 'content-type': 'application/octet-stream' } }))
    .catch(() => undefined);
  return buf;
}
