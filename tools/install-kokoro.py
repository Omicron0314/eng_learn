#!/usr/bin/env python3
"""Install the pinned offline Kokoro runtime in the current user's data directory.

Requires uv; no root, cloud keys or running service. Files are hash verified and
published atomically. --assets-from reuses a previously downloaded directory.
"""
import argparse
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import urllib.request

REV = '1939ad2a8e416c0acfeecc08a694d14ef25f2231'
BASE = f'https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX/resolve/{REV}/'
FILES = {
    'model_q8f16.onnx': (BASE + 'onnx/model_q8f16.onnx', '04c658aec1b6008857c2ad10f8c589d4180d0ec427e7e6118ceb487e215c3cd0'),
    'af_heart.bin': (BASE + 'voices/af_heart.bin', 'd583ccff3cdca2f7fae535cb998ac07e9fcb90f09737b9a41fa2734ec44a8f0b'),
    'bf_emma.bin': (BASE + 'voices/bf_emma.bin', '669fe0647f9dd04fcab92f1439a40eeb4c8b4ab1f82e4996fe3d918ce4a63b73'),
    'config.json': ('https://huggingface.co/hexgrad/Kokoro-82M/resolve/f3ff3571791e39611d31c381e3a41a3af07b4987/config.json', '5abb01e2403b072bf03d04fde160443e209d7a0dad49a423be15196b9b43c17f'),
}


def checksum(path):
    with path.open('rb') as file:
        return hashlib.file_digest(file, 'sha256').hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--assets-from', type=Path)
    args = parser.parse_args()
    base = Path(os.environ.get('XDG_DATA_HOME', ''))
    if not base.is_absolute():
        base = Path.home() / '.local/share'
    root = base / 'cijian/kokoro'
    assets = root / 'assets'
    assets.mkdir(parents=True, exist_ok=True)
    for name, (url, expected) in FILES.items():
        dest = assets / name
        if dest.exists() and checksum(dest) == expected:
            continue
        with tempfile.NamedTemporaryFile(dir=assets, delete=False) as tmp:
            temporary = Path(tmp.name)
        try:
            if args.assets_from:
                shutil.copyfile(args.assets_from / name, temporary)
            else:
                print(f'Downloading {name}', flush=True)
                with urllib.request.urlopen(url, timeout=60) as response, temporary.open('wb') as output:
                    shutil.copyfileobj(response, output)
            if checksum(temporary) != expected:
                raise RuntimeError(f'Checksum mismatch: {name}')
            temporary.replace(dest)
        finally:
            temporary.unlink(missing_ok=True)
    venv = root / 'venv'
    if not (venv / 'bin/python').exists():
        subprocess.run(['uv', 'venv', '--python', '3.12', str(venv)], check=True)
    subprocess.run(['uv', 'pip', 'sync', '--python', str(venv / 'bin/python'),
                    str(Path(__file__).with_name('kokoro-requirements.txt'))], check=True)
    # Use original text, never user documents, to exercise both voices on install.
    helper = Path(__file__).resolve().parent.parent / 'crates/cijian-platform/src/kokoro.py'
    for voice in ('af_heart', 'bf_emma'):
        with tempfile.TemporaryDirectory() as temp:
            subprocess.run([str(venv / 'bin/python'), str(helper), str(assets), voice,
                            str(Path(temp) / 'check.wav')],
                           input='This original sentence verifies offline English speech.',
                           text=True, check=True, timeout=180)
    print(f'Installed and verified both voices: {root}')


if __name__ == '__main__':
    main()
