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
use arora_types::gen_uuid_from_str;
use arora_types::record::module::frozen::{Function, Parameter};
use arora_types::record::ty::{FrozenScalar, FrozenTy, PrimitiveKind};
use arora_types::record::{FrozenReference, Version};
use uuid::Uuid;

/// The rest token, written whenever nothing is speaking (both vocabularies).
pub const SILENCE_VISEME: &str = "sil";

/// The `say` function's id — identical across providers.
pub fn say_id() -> Uuid {
    gen_uuid_from_str("say")
}

pub fn text_param_id() -> Uuid {
    gen_uuid_from_str("say.text")
}
pub fn voice_param_id() -> Uuid {
    gen_uuid_from_str("say.voice")
}
pub fn viseme_param_id() -> Uuid {
    gen_uuid_from_str("say.viseme")
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
