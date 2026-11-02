# R00 平台原型

独立消费者，不读取真实学习库。锁定版本、验收结果见 [执行记录](../../docs/spikes/R00-platform.md)。

```sh
cargo build --manifest-path spikes/r00/Cargo.toml --locked
cargo run --manifest-path spikes/r00/Cargo.toml --locked
python3 spikes/r00/check-dependencies.py
cargo test --manifest-path spikes/r00/fsrs/Cargo.toml --locked
cargo test --manifest-path spikes/r00/native/Cargo.toml --locked
cargo run --manifest-path spikes/r00/native/Cargo.toml --locked -- --audio
# 仅打开选择器，不读取所选内容
cargo run --manifest-path spikes/r00/native/Cargo.toml --locked -- --file-dialog
```

WASM 原型：

```sh
cargo build --manifest-path spikes/r00/Cargo.toml --lib --locked --target wasm32-unknown-unknown
# 使用与锁文件一致的 wasm-bindgen-cli 0.2.130；可安装到独立工具目录
cargo install wasm-bindgen-cli --version 0.2.130 --locked --root /tmp/cijian-r00-tools
/tmp/cijian-r00-tools/bin/wasm-bindgen --target web --out-dir spikes/r00/web/pkg spikes/r00/target/wasm32-unknown-unknown/debug/cijian_platform_spike.wasm
python3 -m http.server 4319 --bind 127.0.0.1
# 浏览器打开 http://127.0.0.1:4319/spikes/r00/web/
```

`web/storage-probe.js` 在随机命名的临时 IndexedDB 中测试提交、回滚、关闭重开，并在 finally 删除该测试库。`web/cors-fixture.py` 是本机4320端口的无数据CORS样例。所有浏览器测试都应使用独立临时浏览器上下文。

字体：`assets/DejaVuSans.ttf` 是本机 DejaVu Sans 的未修改副本，授权见同目录文件；`CijianProbePunctuation.otf` 是 Noto Sans CJK SC 的 U+3000–303F、U+FF00–FFEF 子集，已改名，原授权保留在 `NotoSansCJK-LICENSE.txt`。这些仅用于消费者验证，不等同于产品的Roboto Flex品牌字体。

`export-fsrs.mjs` 从本项目安装的 ts-fsrs 5.4.2 导出原创固定向量；运行需现有 pnpm 依赖。`fsrs-comparison.json` 只比较记忆状态；正式领域测试 `crates/cijian-domain/tests/legacy_vectors.rs` 比较全部调度字段。
