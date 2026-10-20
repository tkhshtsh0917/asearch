use asearch_rs::{Asearch, Profile};
use wasm_bindgen::prelude::*;
mod index;

#[wasm_bindgen]
pub struct AsearchHandle(Asearch);

#[wasm_bindgen]
impl AsearchHandle {
    #[wasm_bindgen(constructor)]
    pub fn new(
        key_cps: &[u32],
        key_masks: &[u32],
        loop_mask: u32,
        accept: u32,
        init: u32,
        profile: u8,
    ) -> Result<AsearchHandle, JsError> {
        if key_cps.len() != key_masks.len() {
            return Err(JsError::new("Mask arrays have different lengths"));
        }
        let profile = match profile {
            0 => Profile::AsearchTs,
            1 => Profile::AsearchTsLib,
            2 => Profile::Ideal,
            _ => return Err(JsError::new("Unknown profile")),
        };
        let masks: Vec<_> = key_cps
            .iter()
            .copied()
            .zip(key_masks.iter().copied())
            .collect();
        Ok(Self(Asearch::from_parts(
            &masks, loop_mask, accept, init, profile,
        )))
    }

    pub fn match_level_u16(&self, text: &[u16], max_ambig: u16) -> Result<i32, JsError> {
        self.0
            .match_level_utf16(text, max_ambig)
            .map(|level| level.map_or(-1, i32::from))
            .map_err(|error| JsError::new(&error.to_string()))
    }

    pub fn match_u16(&self, text: &[u16], ambig: u16) -> bool {
        self.0.is_match_utf16(text, ambig)
    }
}
