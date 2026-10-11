export type TTSResult = { audio: Uint8Array; mimeType: string; durationS: number | null };

export interface TTSProvider {
  readonly provider: string;
  readonly model: string;
  readonly voice: string;
  synthesize(req: { text: string }): Promise<TTSResult>;
}
