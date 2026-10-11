import type { LLMProvider, LLMRequest, Price } from "@eng-learn/providers";
/** Reserve a conservative per-request bound BEFORE IO, including repair/fallback requests.
 * UTF-8 bytes + per-message overhead upper-bound prompt tokens; output is provider-capped.
 * Failed calls retain their reservation because providers may have consumed tokens.
 */
export function costGuard(provider: LLMProvider, price: Price, maxCost: number) {
  let reserved = 0;
  const maxTokens = 2048;
  const guarded: LLMProvider = {
    provider: provider.provider,
    model: provider.model,
    async complete(request: LLMRequest) {
      const inputBound = request.messages.reduce(
        (n, message) => n + Buffer.byteLength(message.content, "utf8") + 128,
        256,
      );
      const bound =
        (inputBound / 1e6) * price.inputPerMTok + (maxTokens / 1e6) * price.outputPerMTok;
      if (reserved + bound > maxCost) throw new Error("EVAL_BUDGET_EXHAUSTED");
      reserved += bound;
      return provider.complete({ ...request, maxTokens });
    },
  };
  return { provider: guarded, reserved: () => reserved };
}
