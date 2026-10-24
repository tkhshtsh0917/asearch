# Native Japanese analysis

[日本語](ja/analysis.md)

`asearch-analyzer` is a server-only crate for Sudachi tokenization, filtering, directed synonyms and romanized readings. Only `asearch-cli` depends on it. `asearch-wasm`, `asearch-engine` and `asearch-builder` do not load a tokenizer or language dictionary. ASIX v1 remains unchanged: a document's existing `texts` carry its original and generated alternatives.

```sh
asearch-cli compile-analysis-resources stopwords.txt stoptags.txt synonyms.txt resources.json
asearch-cli analyze system.dic resources.json policy.json questions.jsonl analyzed.jsonl
asearch-cli build analyzed.jsonl index.asix
```

Analysis input is JSONL with one original text per key, for example `{"key":1,"texts":["ビートルズの曲は？"]}`. `order` is optional. Output is ordinary builder JSONL with the original first and additional texts. The host retains display text, URLs and attribution in a separate key-addressed catalog. Synonym terms are independent alternatives, not copies of the whole question; combining keywords across different texts is not supported.

The resource compiler removes stop-list comments and unused CSV columns, preserving synonym groups and direction. Empty/`0` flags trigger and emit, `1` only emits, and `2` is excluded. Lookup uses lowercased exact token surface/normalized/dictionary forms, performs one hop and does not enumerate replacement combinations. Stoptags match POS component prefixes. Filtering affects only generated forms; the original remains searchable.

Example policy:

```json
{
  "splitMode": "C",
  "keepPartOfSpeech": ["名詞", "動詞", "形容詞", "形状詞"],
  "maxSynonymTexts": 256,
  "maxDocumentUnits": 131072
}
```

The synonym quota counts only unique texts added after the original, normalized words, reading and romanizations. Repeated entries and synonyms identical to those basic forms do not consume it. Document units count all final texts once.

Either numeric limit accepts `-1` to disable that application limit. Other nonpositive values are rejected. This does not disable ASIX's 65,535 UTF-16-unit capacity per text, the CLI's 1 MiB line/64 MiB JSONL input and 64 MiB output bounds, its 20,000-record analysis bound, or the builder/engine's resource limits. A limit violation fails instead of silently dropping alternatives.

Sudachi also limits the source to 49,149 UTF-8 bytes before its input plugins run, and the rewritten input to 65,535 UTF-8 bytes. These are byte limits, independent of ASIX UTF-16 units, and remain active with `-1`. For example, 16,384 copies of `あ` exceed the source limit even though they fit an ASIX text. Input normalization can expand a shorter source past the rewritten-input limit. An oversized source or rewritten input fails without truncation.

The native Sudachi dependency is fixed to Git revision `1c21940d1ca6b045eb211ecc1cdb767326f3783f`; wana_kana 5.0.0 supplies kana romanization. ビートルズ yields both `biitoruzu` and `bi-toruzu`. English `Beatles` requires a synonym entry rather than romanization. Dictionary coverage and relevance remain application concerns.

The CLI uses Sudachi's embedded built-in plugins and an explicit system dictionary path. Sudachi memory-maps that dictionary. Keep the mapped file immutable for the analyzer's lifetime; content-addressed files and atomic replacement of pointers support safe updates. Compiled stop/synonym resources are small structured JSON loaded once into owned Rust collections; an additional mmap layer is unnecessary here.

The 20,000-record limit is per analysis invocation, not a maximum ASIX document count. Larger corpora can concatenate analyzed batches before `build`; the builder and client must still have sufficient text/unit/memory budgets. The demo's 50k probe produced 574,363 searchable texts and a 40,632,640-byte base; static loading/search passed with a 256 MiB client working budget. That does not establish total browser RAM usage or update-transition headroom.

Network retrieval and upstream revision/hash management belong to the host. The [demo's analysis pipeline](https://github.com/tkhshtsh0917/asearch-demo/blob/main/docs/search-analysis.md) provides explicit latest-resource updates, pinned ordinary builds, integrity checks, immutable dictionary caching, attribution and browser regression tests. Native source/fixture licenses are included with Release Assets under `licenses/server-dependencies/`.
