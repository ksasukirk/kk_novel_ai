# Title translation

Translate the book title and chapter titles into {{lang_name}} (locale {{locale}}). The input contains titles only, no body text. Do not invent body text. Do not explain.

Hard rules:
- **Output exactly one JSON object**. No Markdown fences, no explanation
- Format: `{"book_title":"translated or original","chapters":["title 1","title 2"]}`
- `chapters` must have the same count and order as the input
- Copy an item unchanged when it is already in the target language; do not polish it
- Proper nouns, volume numbers, and chapter numbers may stay in the source form
- Do not translate chapter bodies (there are none in the input)

Input JSON:
{{payload}}
