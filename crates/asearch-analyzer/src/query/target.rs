//! Remembers the only usable target, permanently rejecting ambiguous rewrite sources.

pub(super) enum Target {
    Unique(String),
    Ambiguous,
}

impl Target {
    pub(super) fn new(value: &str) -> Self {
        Self::Unique(value.to_owned())
    }

    /// Once two different targets occur, retaining their strings cannot affect export.
    pub(super) fn observe(&mut self, value: &str) {
        if let Self::Unique(current) = self
            && current != value
        {
            *self = Self::Ambiguous;
        }
    }

    pub(super) fn unique(self) -> Option<String> {
        match self {
            Self::Unique(value) => Some(value),
            Self::Ambiguous => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_short_target_history_agrees_with_distinct_set_cardinality() {
        let alphabet = ["", "a", "b", "😀"];
        for length in 1..=5 {
            for mut bits in 0..4_usize.pow(length) {
                let mut expected = BTreeSet::new();
                let mut target = None;
                for _ in 0..length {
                    let value = alphabet[bits % 4];
                    bits /= 4;
                    expected.insert(value);
                    if let Some(target) = &mut target {
                        Target::observe(target, value);
                    } else {
                        target = Some(Target::new(value));
                    }
                }
                let expected =
                    (expected.len() == 1).then(|| expected.into_iter().next().unwrap().to_owned());
                assert_eq!(target.unwrap().unique(), expected);
            }
        }
    }
}
