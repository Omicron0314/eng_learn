"""Offline Kokoro q8f16 helper. stdin is UTF-8 text; argv is assets, voice, WAV.

Uses the pinned ONNX export directly: its token input is input_ids, unlike
some other Kokoro exports. No network access or automatic model downloads.
"""
import json
import sys
import wave
from pathlib import Path

import numpy as np
import onnxruntime as ort
from kokoro_onnx.tokenizer import Tokenizer


def chunks(phonemes, limit=200):
    """Prefer punctuation/word boundaries; preserve every phoneme exactly once."""
    while phonemes:
        if len(phonemes) <= limit:
            yield phonemes
            return
        end = max(phonemes.rfind(mark, 0, limit) + 1 for mark in '.!?;\n')
        if end < limit // 2:
            end = phonemes.rfind(' ', 0, limit) + 1
        if end <= 0:
            end = limit
        yield phonemes[:end]
        phonemes = phonemes[end:]


def synthesize(assets, voice, text, output):
    if voice not in ('af_heart', 'bf_emma') or not text.strip():
        raise ValueError('Invalid voice or empty text')
    config = json.loads((assets / 'config.json').read_text())
    tokenizer = Tokenizer(vocab=config['vocab'])
    options = ort.SessionOptions()
    options.intra_op_num_threads = 4
    options.inter_op_num_threads = 1
    options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_DISABLE_ALL
    session = ort.InferenceSession(str(assets / 'model_q8f16.onnx'), options,
                                   providers=['CPUExecutionProvider'])
    styles = np.fromfile(assets / (voice + '.bin'), dtype=np.float32).reshape(-1, 1, 256)
    input_types = {item.name: item.type for item in session.get_inputs()}
    def dtype(name):
        return np.float16 if input_types[name] == 'tensor(float16)' else np.float32
    total = 0
    with wave.open(str(output), 'wb') as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(24000)
        # Paragraphs bound phonemizer memory; empty paragraphs do not discard text.
        for paragraph in text.splitlines():
            if not paragraph.strip():
                continue
            phonemes = tokenizer.phonemize(paragraph, lang='en-us' if voice == 'af_heart' else 'en-gb')
            for chunk in chunks(phonemes):
                tokens = [config['vocab'][p] for p in chunk if p in config['vocab']]
                if not tokens:
                    continue
                samples = session.run(None, {
                    'input_ids': np.array([[0, *tokens, 0]], dtype=np.int64),
                    'style': styles[len(tokens) - 1].astype(dtype('style')),
                    'speed': np.array([1.0], dtype=dtype('speed')),
                })[0].reshape(-1)
                if not np.isfinite(samples).all() or not len(samples):
                    raise ValueError('Invalid model output')
                total += len(samples)
                if total > 24000 * 60 * 60:
                    raise ValueError('Audio exceeds one hour')
                wav.writeframes((np.clip(samples, -1, 1) * 32767).astype('<i2').tobytes())
        if not total:
            raise ValueError('No speech produced')


if __name__ == '__main__':
    synthesize(Path(sys.argv[1]), sys.argv[2], sys.stdin.read(1024 * 1024 + 1), Path(sys.argv[3]))
