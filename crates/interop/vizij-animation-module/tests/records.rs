//! The module's records, from which a device describes the exports a header
//! names without describing: those taking or returning a structure, such as
//! `step_values` and `player_states`.

use arora_registry::local::LocalRegistry;
use arora_registry::{ReadableRegistry, TypeDefinitionFrozen};
use arora_types::module::low::Executor;
use arora_types::record::{FrozenReference, Version};
use arora_types::AroraType;
use futures::executor::block_on;
use vizij_animation_module::{animation, OutputKeys, PlayerState, StepValues};
use vizij_arora::VIZIJ;

/// The module record, at the header's version, and the records of the
/// structures its functions name land in the `vizij` scope, and the registry
/// holds everything the module record depends on.
#[test]
fn the_module_and_the_structures_it_names_have_records_in_the_vizij_scope() {
    let header = animation::header(Executor {
        name: "wasm".to_string(),
        min_version: None,
        max_version: None,
    });
    let version = Version::from(header.version).0;
    let mut registry = LocalRegistry::new();
    block_on(registry.add_declared_module::<animation::Module>(version.clone()))
        .expect("the module's records are added");

    let module = registry
        .module_at(&animation::ids::MODULE, &version)
        .expect("the module record is held at the header's version");
    assert_eq!(module.parent, VIZIJ.id);
    assert_eq!(
        block_on(registry.resolve_id(&animation::ids::MODULE)).unwrap(),
        "vizij.vizij-animation"
    );
    assert!(
        registry
            .missing_dependencies(&FrozenReference {
                id: animation::ids::MODULE,
                version: version.into(),
            })
            .is_empty(),
        "every record the module record depends on is held"
    );

    for (id, version, name) in [
        (
            StepValues::arora_type_id(),
            StepValues::arora_type_version(),
            "StepValues",
        ),
        (
            PlayerState::arora_type_id(),
            PlayerState::arora_type_version(),
            "PlayerState",
        ),
        (
            OutputKeys::arora_type_id(),
            OutputKeys::arora_type_version(),
            "OutputKeys",
        ),
    ] {
        let Some(TypeDefinitionFrozen::Structure(record)) = registry.type_at(&id, &version.0)
        else {
            panic!("{name}'s record is held at its version");
        };
        assert_eq!(record.name, name);
        assert_eq!(record.parent, VIZIJ.id, "{name} lives in the vizij scope");
    }
}
