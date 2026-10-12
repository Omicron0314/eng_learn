import { CARD_STATES, renderPrompt } from "@eng-learn/core";
import { AppError, loadPrompt } from "@eng-learn/providers";
import { z } from "zod";
import type { AppDeps } from "../deps";
import { jsonCompletion, nonempty, requireCard, singleFlight, validate } from "./shared";

const definitionSchema = z.object({
  meaningZh: z.string().trim().min(1).max(12),
  pos: z.string().max(30).optional(),
});
export function ensureMeaning(deps: AppDeps, cardId: string) {
  return singleFlight(deps, `meaning:${cardId}`, async () => {
    const card = requireCard(deps, cardId);
    if (card.back.meaning) return { meaning: card.back.meaning, source: card.back.meaningSource };
    const { value } = await jsonCompletion(
      deps,
      "definition",
      renderPrompt(loadPrompt("definition.v1"), { word: card.front, example: card.back.example }),
      definitionSchema,
      '{"meaningZh":string,"pos":string}',
    );
    return deps.tx((repos) => {
      const current = requireCard(deps, cardId, repos);
      // A user edit during the request always takes precedence over generated text.
      if (current.back.meaning)
        return { meaning: current.back.meaning, source: current.back.meaningSource };
      const now = deps.clock().getTime();
      repos.cards.updateBack(
        cardId,
        { ...current.back, meaning: value.meaningZh, pos: value.pos, meaningSource: "ai" },
        now,
      );
      repos.events.append(deps.userId, "card_edit", { cardId, fields: ["meaning"] }, now);
      return { meaning: value.meaningZh, source: "ai" as const };
    });
  });
}
export function updateCardText(
  deps: AppDeps,
  input: { cardId: string; meaning?: string | undefined; example?: string | undefined },
) {
  input = validate(
    z
      .object({
        cardId: nonempty,
        meaning: z.string().max(1000).optional(),
        example: z.string().max(5000).optional(),
      })
      .refine((i) => i.meaning !== undefined || i.example !== undefined, "没有修改内容"),
    input,
  );
  deps.tx((repos) => {
    const card = requireCard(deps, input.cardId, repos);
    const now = deps.clock().getTime();
    const fields = (["meaning", "example"] as const).filter(
      (key) => input[key] !== undefined && input[key] !== card.back[key],
    );
    if (!fields.length) return;
    repos.cards.updateBack(
      card.id,
      {
        ...card.back,
        ...(input.meaning !== undefined
          ? { meaning: input.meaning, meaningSource: "user" as const }
          : {}),
        ...(input.example !== undefined ? { example: input.example } : {}),
      },
      now,
    );
    repos.events.append(deps.userId, "card_edit", { cardId: card.id, fields }, now);
  });
}
export function setSuspended(deps: AppDeps, input: { cardId: string; suspended: boolean }) {
  input = validate(z.object({ cardId: nonempty, suspended: z.boolean() }), input);
  deps.tx((repos) => {
    const card = requireCard(deps, input.cardId, repos);
    if (card.suspended === input.suspended) return;
    const now = deps.clock().getTime();
    repos.cards.setSuspended(card.id, input.suspended, now);
    repos.events.append(deps.userId, "card_suspend", input, now);
  });
}
export function listCards(
  deps: AppDeps,
  input: {
    q?: string | undefined;
    deck?: string | undefined;
    state?: string | undefined;
    suspended?: boolean | undefined;
    offset?: number | undefined;
    limit?: number | undefined;
  } = {},
) {
  const filter = validate(
    z.object({
      q: z.string().max(1000).optional(),
      deck: nonempty.optional(),
      state: z.enum(CARD_STATES).optional(),
      suspended: z.boolean().optional(),
      offset: z.number().int().min(0).default(0),
      limit: z.number().int().min(1).max(100).default(50),
    }),
    input,
  );
  return deps.repos.cards.search(deps.userId, filter);
}
export function deleteDeck(
  deps: AppDeps,
  input: { deck: string },
): { deck: string; deleted: number; keptWithHistory: number } {
  const { deck } = validate(z.object({ deck: nonempty }).strict(), input);
  return deps.tx((repos) => {
    const ids = repos.cards.idsByDeck(deps.userId, deck);
    if (!ids.length) throw new AppError("NOT_FOUND", `牌组不存在：${deck}`, 404);
    const now = deps.clock().getTime();
    let deleted = 0;
    let keptWithHistory = 0;
    for (const cardId of ids) {
      // review_logs are append-only with an FK to cards: cards that have review
      // history cannot be hard-deleted, so they are suspended instead.
      if (repos.reviewLogs.countByCard(cardId) > 0) {
        repos.cards.setSuspended(cardId, true, now);
        keptWithHistory++;
      } else {
        repos.cards.delete(cardId);
        deleted++;
      }
    }
    repos.events.append(deps.userId, "deck_delete", { deck, deleted, keptWithHistory }, now);
    return { deck, deleted, keptWithHistory };
  });
}
export const cardService = {
  ensureMeaning,
  updateCardText,
  setSuspended,
  deleteDeck,
  listCards,
};
