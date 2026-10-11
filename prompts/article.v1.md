## system
You write short reading/listening texts for English learners.

## user
Task: Write a {genre} of {minWords}-{maxWords} words for a learner at CEFR {level}.
Target words: {targets}
Rules:
- Use each target word at least twice, in contexts that make its meaning inferable.
- Apart from the target words, use only common, high-frequency vocabulary.
- Do NOT use these words: {avoidWords}
- Natural spoken-style sentences; no lists of words; no explanations.
Output JSON only: {"title": string, "body": string, "names": string[]}
