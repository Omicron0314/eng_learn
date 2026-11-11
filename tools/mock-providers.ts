import { createServer } from "node:http";
import type { LLMRequest } from "@eng-learn/providers";
import { silentWav, testResponder } from "@eng-learn/providers/testing";
import { z } from "zod";

/**
 * Test-only OpenAI-compatible endpoint for browser tests and offline tooling.
 * Never imported by the app; the real provider is configured through .env or /settings.
 */
const port = z.coerce
  .number()
  .int()
  .min(1)
  .max(65535)
  .parse(process.env.MOCK_PROVIDER_PORT ?? 3200);

const chatRequest = z
  .object({
    messages: z.array(z.object({ role: z.string(), content: z.string() })).min(1),
    response_format: z.object({ type: z.string() }).optional(),
  })
  .passthrough();

function purposeOf(text: string): LLMRequest["purpose"] {
  if (text.includes("Target words:")) return "article";
  if (text.includes("base form:")) return "gloss";
  if (text.includes('"meaningZh"')) return "definition";
  return "article";
}

const server = createServer(async (request, response) => {
  const url = new URL(request.url ?? "/", `http://127.0.0.1:${port}`);
  if (request.method === "GET" && url.pathname === "/v1/models") {
    response
      .writeHead(200, { "content-type": "application/json" })
      .end(JSON.stringify({ object: "list", data: [] }));
    return;
  }
  if (request.method !== "POST") {
    response.writeHead(404).end();
    return;
  }
  const chunks: Buffer[] = [];
  for await (const chunk of request) chunks.push(Buffer.from(chunk as Uint8Array));
  const raw = Buffer.concat(chunks).toString("utf8");
  if (url.pathname === "/v1/audio/speech") {
    const audio = silentWav(1);
    response
      .writeHead(200, {
        "content-type": "audio/wav",
        "content-length": String(audio.length),
      })
      .end(Buffer.from(audio));
    return;
  }
  if (url.pathname !== "/v1/chat/completions") {
    response.writeHead(404).end();
    return;
  }
  try {
    const parsed = chatRequest.parse(JSON.parse(raw));
    const text = parsed.messages.map((m) => m.content).join("\n");
    const purpose = purposeOf(text);
    const content = testResponder({
      purpose,
      messages: parsed.messages as LLMRequest["messages"],
      ...(parsed.response_format ? { jsonMode: true } : {}),
    });
    const body = {
      id: `mock-${Date.now()}`,
      object: "chat.completion",
      created: Math.floor(Date.now() / 1000),
      model: "mock-model",
      choices: [{ index: 0, message: { role: "assistant", content }, finish_reason: "stop" }],
      usage: { prompt_tokens: 1, completion_tokens: 1, total_tokens: 2 },
    };
    response.writeHead(200, { "content-type": "application/json" }).end(JSON.stringify(body));
  } catch (error) {
    response
      .writeHead(400, { "content-type": "application/json" })
      .end(JSON.stringify({ error: { message: (error as Error).message } }));
  }
});

server.listen(port, "127.0.0.1", () => {
  console.log(`mock providers listening on http://127.0.0.1:${port}/v1`);
});
