# Extension guests

Extensions are WebAssembly components implementing the `pane:extension`
contract in [`wit/extension.wit`](../wit/extension.wit). Pane registers only
WASI 0.3 interfaces; a component that imports WASI 0.2 (for example through
Rust's standard library on `wasm32-wasip2`) is rejected with an explanation.

- `pane-guest`: Rust bindings for the contract. `no_std`, so only WASI 0.3 is
  imported; it supplies the allocator, a trapping panic handler and
  `cabi_realloc`.
- `sample-rust`: the Rust sample command Pane opens by default.
- `fixtures/faulty`: test fixture whose actions return an error or trap.
- `fixtures/mixed-p2`: negative control that imports WASI 0.2 and must be rejected.

## Writing a Rust command

The [sample](sample-rust/src/lib.rs) is the complete example. A command is a
`cdylib` crate depending on `pane-guest` that implements two async functions:

```rust
#![no_std]

use pane_guest::alloc::{string::String, vec};
use pane_guest::{Guest, Item, View};

struct Hello;
pane_guest::export!(Hello);

impl Guest for Hello {
    async fn get_view() -> Result<View, String> {
        Ok(View {
            title: "Hello".into(),
            items: vec![Item { id: "hi".into(), title: "Say hi".into(), subtitle: None }],
        })
    }

    async fn run_action(_item_id: String) -> Result<String, String> {
        Ok("hi!".into())
    }
}
```

Returning `Err` shows the message as an error; a panic traps the guest, which
Pane reports and recovers from by starting a fresh instance on the next call.
WASI 0.3 interfaces are available through the
[`wasip3`](https://docs.rs/wasip3/0.9.0/wasip3/) crate with
`default-features = false`; the sample awaits `wasi:clocks` this way.

Build with the pinned toolchain (`wasm32-wasip2` is the compiler target name;
the emitted component imports only WASI 0.3):

```sh
cargo xtask guests
# or, for the guests workspace only:
cd guests && cargo build --release --target wasm32-wasip2
```

To try a rebuilt sample in the launcher, `cargo run -p pane` from the root; set
`PANE_EXTENSIONS_DIR` to a directory containing `sample_rust.wasm` to use a
different build. Installing arbitrary extensions is not supported yet.

Toolchain used: Rust 1.98.1, `wit-bindgen` 0.62.0, `wasip3` 0.9.0+wasi-0.3.0;
host Wasmtime and wasmtime-wasi 49.0.1.
