use crate::{Asearch, Profile};

impl Asearch {
    pub fn is_match(&self, text: &str, ambig: u16) -> bool {
        match self.profile {
            Profile::AsearchTs | Profile::Ideal => {
                self.match_symbols(text.chars().map(u32::from), ambig)
            }
            Profile::AsearchTsLib => self.match_symbols(text.encode_utf16().map(u32::from), ambig),
        }
    }

    pub fn is_match_utf16(&self, text: &[u16], ambig: u16) -> bool {
        match self.profile {
            Profile::AsearchTs | Profile::Ideal => self.match_symbols(
                char::decode_utf16(text.iter().copied())
                    .map(|c| c.map_or_else(|e| u32::from(e.unpaired_surrogate()), u32::from)),
                ambig,
            ),
            Profile::AsearchTsLib => self.match_symbols(text.iter().copied().map(u32::from), ambig),
        }
    }

    fn match_symbols(&self, text: impl Iterator<Item = u32>, ambig: u16) -> bool {
        let count = usize::from(ambig) + 1;
        if count <= 32 {
            let mut stack = [0u32; 32];
            self.run(text, &mut stack[..count])
        } else {
            // This deliberately slow compatibility path preserves JS shifts
            // above 31. Ordinary matching never allocates this buffer.
            self.run(text, &mut vec![0; count])
        }
    }

    fn run(&self, text: impl Iterator<Item = u32>, states: &mut [u32]) -> bool {
        self.advance(text, states);
        states.last().is_some_and(|state| state & self.accept != 0)
    }

    pub(crate) fn advance(&self, text: impl Iterator<Item = u32>, states: &mut [u32]) {
        let mut prefix = 0;
        for (level, state) in states.iter_mut().enumerate() {
            *state = if self.profile == Profile::Ideal {
                prefix |= self.init.checked_shr(level as u32).unwrap_or(0);
                prefix
            } else {
                self.init.wrapping_shr(level as u32)
            };
        }
        for symbol in text {
            let mask = self
                .masks
                .binary_search_by_key(&symbol, |&(key, _)| key)
                .map_or(0, |i| self.masks[i].1);
            for e in (1..states.len()).rev() {
                states[e] = (states[e] & self.loop_mask)
                    | ((states[e] & mask) >> 1)
                    | (states[e - 1] >> 1)
                    | states[e - 1];
            }
            states[0] = ((states[0] & mask) >> 1) | (states[0] & self.loop_mask);
            for e in 1..states.len() {
                states[e] |= states[e - 1] >> 1;
            }
        }
    }
}
