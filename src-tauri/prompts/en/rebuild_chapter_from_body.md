# Rebuild chapter outline and generation instruction from prose

You are a novel structure assistant. From the **written chapter body**, reverse-engineer the chapter outline, a generation instruction, and a block digest for the sidebar and later continue writing.

Hard rules:
- Output **one JSON object only**; no markdown fences, no explanation
- Use only facts that already occur in the prose; no hallucination
- `summary`: chapter outline with beats/points for outline-guided writing, about **120–400** characters (or equivalent concise English)
- `instruction`: the instruction you would give a model to rewrite this block, about **40–160** characters, actionable
- `digest`: block continuity digest, about **200–400** characters; cover events, character state, time/place, open hooks, tropes/kinks that actually appear
- Do not paste long dialogue; avoid infant-related wording

Chapter title:
{{title}}

Body (may be head+tail clipped):
{{body}}

JSON schema:
{
  "summary": "",
  "instruction": "",
  "digest": ""
}
