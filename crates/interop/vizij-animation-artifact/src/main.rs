//! Write arora-animation-module's module directory, as a device loads the
//! guest from it: `header.json`, `records/` (the module's record and the
//! records of the types its functions name, in the `arora` scope) and, when
//! its path is given, the guest built for `wasm32-wasip1`. `@vizij/runtime`'s
//! tests load it (`tests/animation-guest.mjs`): a Vizij device host-links the
//! same module, so it is the guest at hand for proving the `modules` route.
//!
//! ```text
//! cargo build -p arora-animation-module --target wasm32-wasip1
//! cargo run -q -p vizij-animation-artifact -- <dir> target/wasm32-wasip1/debug/arora_animation_module.wasm
//! ```

use std::path::PathBuf;

fn main() {
    let mut args = std::env::args_os().skip(1);
    let dir = PathBuf::from(
        args.next()
            .expect("usage: vizij-animation-artifact <dir> [<guest .wasm>]"),
    );
    let guest = args.next().map(PathBuf::from);
    arora_animation_module::module_dir::write(&dir, guest.as_deref())
        .expect("the module directory is written");
}
