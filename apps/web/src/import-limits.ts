const MIB = 1024 * 1024;
export const TEXT_IMPORT_MAX_BYTES = 20 * MIB;
export const APKG_IMPORT_MAX_BYTES = 64 * MIB;
export const MULTIPART_IMPORT_MAX_BYTES = APKG_IMPORT_MAX_BYTES + 64 * 1024;
export const PROXY_BODY_MAX_BYTES = 65 * MIB;
export function importFileMaxBytes(fileName: string): number {
  return /\.apkg$/i.test(fileName) ? APKG_IMPORT_MAX_BYTES : TEXT_IMPORT_MAX_BYTES;
}
