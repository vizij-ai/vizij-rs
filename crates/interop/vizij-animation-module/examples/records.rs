//! Write the module's Arora records — its module record and the records of
//! the structures its functions take and return, in the `vizij` scope — into
//! a directory, in the layout a device loads them from: what
//! `npm/@vizij/animation-module`'s artifact build ships as `records/`.
//!
//! ```text
//! cargo run -q -p vizij-animation-module --example records -- <directory>
//! ```

use arora_registry::local::LocalRegistry;
use arora_registry::local_yaml::write_records_to_yaml_dir;
use arora_types::module::low::Executor;
use arora_types::record::Version;
use vizij_animation_module::animation;

fn main() {
    let dir = std::env::args_os()
        .nth(1)
        .expect("usage: records <directory>");
    let header = animation::header(Executor {
        name: "wasm".to_string(),
        min_version: None,
        max_version: None,
    });
    let mut registry = LocalRegistry::new();
    futures::executor::block_on(
        registry.add_declared_module::<animation::Module>(Version::from(header.version).0),
    )
    .expect("the module's records are added");
    write_records_to_yaml_dir(&registry, dir).expect("the records are written");
}
