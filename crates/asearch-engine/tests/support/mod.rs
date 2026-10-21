use asearch_engine::MatchMode;

// Deliberately uses a cost matrix rather than the product's state recurrence.
pub fn oracle(pattern: &str, text: &str, mode: MatchMode) -> usize {
    let mut letters = Vec::new();
    let mut loops = vec![mode == MatchMode::Include];
    for c in pattern.chars() {
        if mode == MatchMode::WildcardSpace && c == ' ' {
            *loops.last_mut().unwrap() = true;
        } else {
            letters.push(c);
            loops.push(false);
        }
    }
    if mode == MatchMode::Include {
        *loops.last_mut().unwrap() = true;
    }
    let text: Vec<_> = text.chars().collect();
    let mut costs = vec![vec![0; text.len() + 1]; letters.len() + 1];
    for i in 0..=letters.len() {
        costs[i][0] = i;
        for j in 1..=text.len() {
            costs[i][j] = costs[i][j - 1] + usize::from(!loops[i]);
            if i > 0 {
                let c = letters[i - 1];
                let t = text[j - 1];
                let same = c == t
                    || c.to_lowercase().eq(std::iter::once(t))
                    || c.to_uppercase().eq(std::iter::once(t));
                costs[i][j] = costs[i][j]
                    .min(costs[i - 1][j - 1] + usize::from(!same))
                    .min(costs[i - 1][j] + 1);
            }
        }
    }
    costs[letters.len()][text.len()]
}
