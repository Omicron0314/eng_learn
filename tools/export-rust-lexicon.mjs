// Export the existing licensed WordNet-derived assets, never a user deck.
import { createRequire } from 'node:module';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
const root = createRequire(new URL('../packages/core/package.json', import.meta.url));
const lemmatizer = root.resolve('wink-lemmatizer');
const require = createRequire(lemmatizer);
const directory = dirname(require.resolve('wink-lexicon/package.json'));
if (JSON.parse(readFileSync(resolve(directory,'package.json'),'utf8')).version !== '2.2.0') throw new Error('Review lexicon version before exporting');
const words=require('wink-lexicon/src/wn-words.js');
const senses=require('wink-lexicon/src/wn-word-senses.js');
const out=new URL('../crates/cijian-domain/assets/lexicon/',import.meta.url);
mkdirSync(out,{recursive:true});
writeFileSync(new URL('words.tsv',out),Object.keys(words).sort().map(word=>{
 const list=senses[words[word]];
 const flags=(list.some(s=>s<2)?1:0)|(list.some(s=>s>2&&s<29)?2:0)|(list.some(s=>s>28&&s<44)?4:0);
 return `${word}\t${flags}\n`;
}).join(''));
for(const type of ['verb','noun','adjective']){
 const values=require(`wink-lexicon/src/wn-${type}-exceptions.js`);
 writeFileSync(new URL(`${type}.tsv`,out),Object.entries(values).sort(([a],[b])=>a<b?-1:a>b?1:0).map(([word,lemma])=>`${word}\t${lemma}\n`).join(''));
}
const source=readFileSync(resolve(directory,'src/wn-words.js'),'utf8');
writeFileSync(new URL('NOTICE.txt',out),source.slice(0,source.indexOf('/* eslint'))+'\nMorphology adaptation: wink-lemmatizer 3.0.4, MIT.\n'+readFileSync(resolve(dirname(lemmatizer),'../LICENSE'),'utf8'));
