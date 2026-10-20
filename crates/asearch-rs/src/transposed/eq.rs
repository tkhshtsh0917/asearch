use super::capacity::Budget;
use crate::SearchError;

const CHUNK: usize = 65_536;
const BITMAP_THRESHOLD: usize = 4_096;
const CHUNK_WORDS: usize = CHUNK / 64;

#[derive(Clone, Copy, Debug)]
struct Container {
    id: u32,
    chunk: u32,
    start: usize,
    len: usize,
    bitmap: bool,
}

#[derive(Debug)]
pub(super) struct EqTable {
    alphabet: Vec<u32>,
    containers: Vec<Container>,
    offsets: Vec<u16>,
    bitmaps: Vec<u64>,
}

/// Sorted (code value, absolute slot) entries are temporary build data.
impl EqTable {
    pub fn build(entries: &mut [(u32, u32)], budget: &mut Budget) -> Result<Self, SearchError> {
        entries.sort_unstable();
        let mut symbols = 0;
        let mut containers = 0;
        let mut offsets = 0;
        let mut bitmap_words = 0;
        let mut previous = None;
        for group in entries.chunk_by(|a, b| a.0 == b.0 && a.1 / 65_536 == b.1 / 65_536) {
            let symbol = group[0].0;
            if previous != Some(symbol) {
                symbols += 1;
                previous = Some(symbol);
            }
            containers += 1;
            if group.len() > BITMAP_THRESHOLD {
                bitmap_words += CHUNK_WORDS;
            } else {
                offsets += group.len();
            }
        }

        let mut table = Self {
            alphabet: budget.reserve(symbols)?,
            containers: budget.reserve(containers)?,
            offsets: budget.reserve(offsets)?,
            bitmaps: budget.reserve(bitmap_words)?,
        };
        for group in entries.chunk_by(|a, b| a.0 == b.0 && a.1 / 65_536 == b.1 / 65_536) {
            let symbol = group[0].0;
            if table.alphabet.last() != Some(&symbol) {
                table.alphabet.push(symbol);
            }
            let bitmap = group.len() > BITMAP_THRESHOLD;
            let start = if bitmap {
                table.bitmaps.len()
            } else {
                table.offsets.len()
            };
            if bitmap {
                table.bitmaps.resize(start + CHUNK_WORDS, 0);
                for &(_, slot) in group {
                    let offset = slot as usize % CHUNK;
                    table.bitmaps[start + offset / 64] |= 1 << (offset % 64);
                }
            } else {
                table
                    .offsets
                    .extend(group.iter().map(|&(_, slot)| slot as u16));
            }
            table.containers.push(Container {
                id: table.alphabet.len() as u32,
                chunk: group[0].1 / 65_536,
                start,
                len: group.len(),
                bitmap,
            });
        }
        Ok(table)
    }

    pub fn id(&self, symbol: u32) -> u32 {
        self.alphabet
            .binary_search(&symbol)
            .map_or(0, |i| i as u32 + 1)
    }

    /// ORs one symbol's bits into an already cleared block EQ buffer.
    pub fn scatter(&self, id: u32, first_word: usize, output: &mut [u64]) {
        if id == 0 || output.is_empty() {
            return;
        }
        let first_chunk = (first_word / CHUNK_WORDS) as u32;
        let last_chunk = ((first_word + output.len() - 1) / CHUNK_WORDS) as u32;
        let begin = self
            .containers
            .partition_point(|c| (c.id, c.chunk) < (id, first_chunk));
        for c in &self.containers[begin..] {
            if c.id != id || c.chunk > last_chunk {
                break;
            }
            let chunk_word = c.chunk as usize * CHUNK_WORDS;
            if c.bitmap {
                let lo = first_word.saturating_sub(chunk_word);
                let hi = (first_word + output.len() - chunk_word).min(CHUNK_WORDS);
                for w in lo..hi {
                    output[chunk_word + w - first_word] |= self.bitmaps[c.start + w];
                }
            } else {
                let data = &self.offsets[c.start..c.start + c.len];
                let first_slot = first_word.saturating_sub(chunk_word) * 64;
                let start = data.partition_point(|&offset| (offset as usize) < first_slot);
                for &offset in &data[start..] {
                    let word = chunk_word + offset as usize / 64;
                    if word >= first_word + output.len() {
                        break;
                    }
                    output[word - first_word] |= 1 << (offset % 64);
                }
            }
        }
    }
}
