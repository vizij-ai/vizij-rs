//! The `say` contract shared by every TTS provider module.
//!
//! Providers are sibling host modules with the SAME function id, parameter ids,
//! and signature — a behavior references `say` without caring which provider the
//! build registered (the `tts-piper` feature swaps them; exactly one exists per
//! build). Only the module id differs, so a bridge can still tell providers
//! apart in DescribeMethods.
//!
//! The `viseme` out-parameter is a string whose vocabulary is the provider's
//! (AWS Polly viseme codes for the cloud provider, espeak-ng phonemes for
//! Piper); mapping it to face poses is the caller's job. Both providers write
//! [`SILENCE_VISEME`] at rest.

use std::collections::HashMap;

use arora_behavior_tree_types::STATUS_ENUMERATION_ID;
use arora_types::record::module::frozen::{Function, Parameter};
use arora_types::record::ty::{FrozenScalar, FrozenTy, PrimitiveKind};
use arora_types::record::{FrozenReference, Version};
use uuid::{uuid, Uuid};

/// The rest token, written whenever nothing is speaking (both vocabularies).
pub const SILENCE_VISEME: &str = "sil";

/// The `say` function's id — identical across providers.
pub fn say_id() -> Uuid {
    uuid!("77bf2798-e7ce-47c6-a45c-3c2e9ba1837d")
}

pub fn text_param_id() -> Uuid {
    uuid!("881dc182-d4ba-4ea0-9e81-f4eddab6f669")
}
pub fn voice_param_id() -> Uuid {
    uuid!("f56ca142-db46-4c58-bc44-7896c4b54d5c")
}
pub fn viseme_param_id() -> Uuid {
    uuid!("a1fbf58b-bf66-44a6-a503-9d9078ee5755")
}

/// `say(text, voice) -> Status`, with a mutable `viseme` out-parameter. The
/// `Status` return is the task-run marker a bridge exposes as an action.
pub fn say_signature() -> Function {
    let mut parameters = HashMap::new();
    let mut parameter_ordering = Vec::new();
    for (id, name, kind, mutable) in [
        (text_param_id(), "text", PrimitiveKind::String, false),
        (voice_param_id(), "voice", PrimitiveKind::String, false),
        (viseme_param_id(), "viseme", PrimitiveKind::String, true),
    ] {
        parameter_ordering.push(id);
        parameters.insert(
            id,
            Parameter {
                name: name.to_string(),
                ty: FrozenTy::from(kind),
                mutable,
            },
        );
    }
    Function {
        parameters,
        parameter_ordering,
        return_ty: FrozenTy::FrozenScalar(FrozenScalar {
            reference: FrozenReference {
                id: STATUS_ENUMERATION_ID,
                version: Version::parse("1.0.0").expect("a valid version"),
            },
        }),
    }
}
