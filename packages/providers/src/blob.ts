import { mkdir, readFile, rm, stat, writeFile } from "node:fs/promises";
import path from "node:path";

export type StoredBlob = { data: Uint8Array; contentType: string };

export interface BlobStore {
  put(key: string, data: Uint8Array, contentType: string): Promise<void>;
  get(key: string): Promise<StoredBlob | null>;
  has(key: string): Promise<boolean>;
  delete(key: string): Promise<void>;
}

export function assertSafeKey(key: string): void {
  if (
    !key ||
    key.includes("..") ||
    key.includes("\\") ||
    key.startsWith("/") ||
    !/^[A-Za-z0-9._/-]+$/.test(key)
  ) {
    throw new Error(`Invalid blob key: ${key}`);
  }
}

export class MemoryBlob implements BlobStore {
  private readonly items = new Map<string, StoredBlob>();

  async put(key: string, data: Uint8Array, contentType: string) {
    assertSafeKey(key);
    this.items.set(key, { data: new Uint8Array(data), contentType });
  }
  async get(key: string) {
    assertSafeKey(key);
    return this.items.get(key) ?? null;
  }
  async has(key: string) {
    assertSafeKey(key);
    return this.items.has(key);
  }
  async delete(key: string) {
    assertSafeKey(key);
    this.items.delete(key);
  }
}

export class LocalFileBlob implements BlobStore {
  constructor(private readonly root: string) {}

  private file(key: string) {
    assertSafeKey(key);
    return path.join(this.root, key);
  }

  async put(key: string, data: Uint8Array, contentType: string) {
    const f = this.file(key);
    await mkdir(path.dirname(f), { recursive: true });
    await writeFile(f, data);
    await writeFile(`${f}.type`, contentType);
  }
  async get(key: string) {
    const f = this.file(key);
    try {
      const [data, type] = await Promise.all([readFile(f), readFile(`${f}.type`, "utf8")]);
      return { data: new Uint8Array(data), contentType: type };
    } catch {
      return null;
    }
  }
  async has(key: string) {
    try {
      await stat(this.file(key));
      return true;
    } catch {
      return false;
    }
  }
  async delete(key: string) {
    const f = this.file(key);
    await rm(f, { force: true });
    await rm(`${f}.type`, { force: true });
  }
}
