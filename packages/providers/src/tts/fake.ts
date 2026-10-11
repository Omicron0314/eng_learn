import type { TTSProvider, TTSResult } from "./types";

const SAMPLE_RATE = 8000;

/** Silent mono 8-bit PCM WAV of the given duration. */
export function silentWav(durationS: number): Uint8Array {
  const samples = Math.max(1, Math.round(durationS * SAMPLE_RATE));
  const buf = new Uint8Array(44 + samples);
  const view = new DataView(buf.buffer);
  const write = (offset: number, s: string) => {
    for (let i = 0; i < s.length; i++) buf[offset + i] = s.charCodeAt(i);
  };
  write(0, "RIFF");
  view.setUint32(4, 36 + samples, true);
  write(8, "WAVE");
  write(12, "fmt ");
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, 1, true);
  view.setUint32(24, SAMPLE_RATE, true);
  view.setUint32(28, SAMPLE_RATE, true);
  view.setUint16(32, 1, true);
  view.setUint16(34, 8, true);
  write(36, "data");
  view.setUint32(40, samples, true);
  buf.fill(128, 44);
  return buf;
}

export function wavDuration(wav: Uint8Array): number {
  const view = new DataView(wav.buffer, wav.byteOffset, wav.byteLength);
  const byteRate = view.getUint32(28, true);
  const dataSize = view.getUint32(40, true);
  return dataSize / byteRate;
}

export class FakeTTS implements TTSProvider {
  readonly provider = "fake";
  calls = 0;

  constructor(
    readonly voice = "fake-voice",
    readonly model = "fake-tts",
    private readonly secondsPerChar = 0.002,
  ) {}

  async synthesize(req: { text: string }): Promise<TTSResult> {
    this.calls++;
    const durationS = Math.min(5, Math.max(1, req.text.length * this.secondsPerChar));
    return { audio: silentWav(durationS), mimeType: "audio/wav", durationS };
  }
}
