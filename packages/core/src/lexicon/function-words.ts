const WORDS = `
a an the
i me my mine myself you your yours yourself yourselves he him his himself she her hers herself
it its itself we us our ours ourselves they them their theirs themselves
this that these those who whom whose which what whatever whoever whichever
there here
be am is are was were been being
have has had having do does did doing done
will would shall should can could may might must ought
not no nor
and or but so yet if then than because although though while whereas unless until since
as whether either neither both
of in on at by for with about against between into through during before after above below
to from up down out off over under again further once
around among along across behind beyond near toward towards upon within without onto
some any each every all few more most other such own same
only very too just also even still already ever never
how when where why
one ones
yes oh ok okay
`;

export const FUNCTION_WORDS: ReadonlySet<string> = new Set(WORDS.split(/\s+/).filter(Boolean));

/** Contraction suffixes and the words they stand for. */
export const CONTRACTION_SUFFIXES: Record<string, string> = {
  "n't": "not",
  "'m": "am",
  "'re": "are",
  "'ve": "have",
  "'ll": "will",
  "'d": "would",
  "'s": "is",
};

/** Irregular negative contractions whose stem is not a word on its own. */
export const IRREGULAR_CONTRACTIONS: Record<string, string> = {
  "won't": "will",
  "can't": "can",
  "shan't": "shall",
  "ain't": "be",
};
