use super::{ScalarError, ScalarWm, ScalarWorkspace};
use crate::BitWord;

impl<W: BitWord> ScalarWm<W> {
    /// Returns the minimum Ideal cost within the workspace's reserved levels.
    /// This operation neither allocates nor changes the compiled pattern.
    pub fn match_level(
        &self,
        text: &str,
        workspace: &mut ScalarWorkspace<W>,
    ) -> Result<Option<u16>, ScalarError> {
        workspace.check_budget(self.owned_bytes, self.limits.max_working_bytes)?;
        let states = &mut workspace.states;
        let mut prefix = W::zero();
        for (level, state) in states.iter_mut().enumerate() {
            if level <= self.length {
                prefix = prefix.union(W::bit(level));
            }
            *state = prefix;
        }

        for symbol in text.chars().map(u32::from) {
            let mask = self
                .masks
                .binary_search_by_key(&symbol, |&(s, _)| s)
                .map_or(W::zero(), |i| self.masks[i].1);
            for e in (1..states.len()).rev() {
                states[e] = states[e]
                    .intersection(self.loops)
                    .union(states[e].intersection(mask).shift())
                    .union(states[e - 1].shift())
                    .union(states[e - 1]);
            }
            states[0] = states[0]
                .intersection(self.loops)
                .union(states[0].intersection(mask).shift());
            for e in 1..states.len() {
                states[e] = states[e].union(states[e - 1].shift());
            }
        }
        Ok(states
            .iter()
            .position(|&state| state.intersection(self.accept) != W::zero())
            .map(|level| level as u16))
    }
}
