# Extract tropes / kinks from this block

You are a novel lore assistant. From "prose just written in this block" and the "existing trope / kink list", find tropes or kinks that this block **actually wrote**, and output JSON for the global library.

Hard rules:
- **Output exactly one JSON object**. No Markdown fences, no explanation
- Format: `{"tropes":[{"kind":"trope|kink","title":"short name","matched_title":"catalog title or empty","content":"writing notes, max 40 chars","keywords":["kw"],"evidence":"prose evidence, max 20 chars"}]}`
- At most **5** tropes; if none, output `{"tropes":[]}`
- `kind` must be `trope` (plot beat) or `kink` (erotic preference / play)
- When it matches the existing list: `title` and `matched_title` must use the catalog name; do not invent a near-synonym
- Near-synonyms, abbreviations, reordered words, and alternate names for the same play all count as a match (e.g. "vacuum miniskirt" → an existing "upskirt exposure" card): set `matched_title` to the catalog title and only add new notes in `content` / `keywords` / `evidence`; do not open a replacement card
- When it does not match: leave `matched_title` empty; `title` should be 2–12 characters; open a new card only when the play is clearly different
- Extract only process that appears in this block's prose; do not extract names mentioned only in the instruction
- Do not extract character names, places, or items; do not restate the whole outline as one entry

Existing tropes / kinks (may be empty):
{{known_tropes}}

This turn's instruction (may be empty):
{{instruction}}

This block's prose:
{{block_text}}
