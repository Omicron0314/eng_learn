import { readFileSync } from "node:fs";
import { expect, test } from "@playwright/test";

test("protected full learning loop and manifest", async ({ page, request }) => {
  expect((await request.get("/api/today")).status()).toBe(401);
  expect((await request.get("/api/audio/audio/invalid.wav")).status()).toBe(401);
  await page.goto("/today");
  await expect(page).toHaveURL(/login/);
  await page.getByLabel("访问密码").fill("wrong");
  await page.getByRole("button", { name: "解锁学习空间" }).click();
  await expect(page.locator(".notice.error")).toContainText("密码不正确");
  await page.getByLabel("访问密码").fill("test-password");
  await page.getByRole("button", { name: "解锁学习空间" }).click();
  await expect(page).toHaveURL(/today/);
  await page.goto("/library");
  await page.getByLabel("导入文件").setInputFiles({
    name: "words.csv",
    mimeType: "text/csv",
    buffer: Buffer.from(
      "word,meaning,example\nlearn,学习,I learn every day.\nwalk,走路,I walk home.\nhelp,帮助,I help a friend.\nread,阅读,I read a book.\ntry,尝试,I try again.\nplan,计划,I have a plan.",
    ),
  });
  await page.getByRole("button", { name: "开始导入" }).click();
  await expect(page.getByRole("status")).toContainText("新增 6");
  await page.getByLabel("搜索词卡").fill("learn");
  await page.getByRole("button", { name: "筛选", exact: true }).click();
  await expect(page.getByRole("button", { name: "编辑", exact: true })).toHaveCount(1);
  await page.getByRole("button", { name: "编辑", exact: true }).click();
  await page.getByRole("textbox", { name: "释义", exact: true }).fill("学习（编辑）");
  await page.getByRole("button", { name: "保存词卡" }).click();
  await expect(page.getByRole("table").getByText("学习（编辑）")).toBeVisible();
  await page.getByRole("button", { name: "暂停", exact: true }).click();
  await expect(page.getByRole("button", { name: "恢复", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "恢复", exact: true }).click();
  await page.goto("/review");
  for (let i = 0; i < 6; i++) {
    await page.getByRole("button", { name: "显示答案" }).click();
    await page.getByRole("button", { name: /^4 · Easy/ }).click();
  }
  await expect(page.getByText("今日已完成", { exact: true })).toBeVisible();
  await page.goto("/today");
  await page.getByLabel("体裁").selectOption("story");
  await page.screenshot({ path: "test-results/desktop-today.png", fullPage: true });
  await page.getByRole("button", { name: "生成今日文章" }).click();
  await expect(page).toHaveURL(/article\//, { timeout: 30000 });
  await page.getByRole("button", { name: "显示原文" }).click();
  await page
    .locator("[data-start]")
    .filter({ hasText: /^cold$/ })
    .first()
    .click();
  await expect(page.getByRole("dialog")).toContainText("语境释义（测试）");
  await page.getByRole("button", { name: "标记不认识", exact: true }).click();
  await page.getByRole("button", { name: "关闭弹层" }).click();
  await page
    .locator("[data-start]")
    .filter({ hasText: /^morning$/ })
    .first()
    .click();
  await page.getByRole("button", { name: "标记不认识", exact: true }).click();
  await page.getByRole("button", { name: "关闭弹层" }).click();
  await page.getByRole("button", { name: "生成音频", exact: true }).click();
  await expect(page.locator("audio")).toBeVisible();
  const articleId = page.url().split("/").at(-1);
  const cached = await page.request.post(`/api/articles/${articleId}/audio`, { data: {} });
  const audioSource = await cached.json();
  expect(audioSource.cacheHit).toBe(true);
  const range = await page.request.get(audioSource.url, { headers: { range: "bytes=0-99" } });
  expect(range.status()).toBe(206);
  expect((await range.body()).length).toBe(100);
  await page.getByLabel("播放速度").selectOption("1.25");
  await expect
    .poll(() => page.locator("audio").evaluate((el: HTMLAudioElement) => el.playbackRate))
    .toBe(1.25);
  await page.getByRole("button", { name: "播放", exact: true }).click();
  await expect(page.getByText("听完后，感觉如何？")).toBeVisible({ timeout: 30000 });
  await page.getByRole("button", { name: "刚好", exact: true }).click();
  await expect(page.getByRole("button", { name: "刚好", exact: true })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  const feedbackView = await (await page.request.get(`/api/articles/${articleId}`)).json();
  expect(feedbackView.feedback.listening).toBe("just_right");
  await page.getByRole("button", { name: "读完了", exact: true }).click();
  const readingFeedback = page.locator(".feedback").filter({ hasText: "读完后，感觉如何？" });
  await readingFeedback.getByRole("button", { name: "刚好", exact: true }).click();
  await expect(readingFeedback.getByRole("button", { name: "刚好", exact: true })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await page.getByRole("button", { name: "收起原文" }).click();
  await page.getByRole("button", { name: "显示原文" }).click();
  await expect(readingFeedback.getByRole("button", { name: "刚好", exact: true })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await page.getByRole("button", { name: "标记不自然", exact: true }).click();
  await page.getByLabel("备注").fill("演示反馈");
  await page.getByRole("button", { name: "提交反馈", exact: true }).click();
  await page.goto("/inbox");
  await page.getByLabel("全选").check();
  await page.getByRole("button", { name: "加入复习", exact: true }).click();
  await expect(page.getByText("没有待处理的生词")).toBeVisible();
  await page.goto("/stats");
  await expect(page.getByRole("heading", { name: "对照实验" })).toBeVisible();
  await expect(page.getByTestId("today-new")).toHaveText("6");
  await page.goto("/settings");
  await page.getByLabel("每日新词上限").fill("5");
  await page.getByRole("button", { name: "保存设置", exact: true }).click();
  await expect(page.getByRole("status")).toContainText("已保存");
  await page.reload();
  await expect(page.getByLabel("每日新词上限")).toHaveValue("5");
  const manifest = await request.get("/manifest.webmanifest");
  expect((await manifest.json()).display).toBe("standalone");
  const forbidden = await page.request.post("/api/articles", {
    headers: { origin: "https://elsewhere.test" },
    data: {},
  });
  expect(forbidden.status()).toBe(403);
});

test("mobile browser fallback, quota, lookup actions, inbox ignore and HTTP validation", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.addInitScript(() => {
    const synthesis = {
      paused: false,
      speak: (utterance: SpeechSynthesisUtterance) => {
        (window as unknown as { spokenRate: number }).spokenRate = utterance.rate;
        setTimeout(() => utterance.onend?.(new Event("end") as SpeechSynthesisEvent), 100);
      },
      cancel: () => {},
      pause: () => {},
      resume: () => {},
    };
    Object.defineProperty(window, "speechSynthesis", { value: synthesis });
  });
  await page.goto("/login");
  await page.getByLabel("访问密码").fill("test-password");
  await page.getByRole("button", { name: "解锁学习空间" }).click();
  await expect(page).toHaveURL(/today/);
  await page.request.post("/api/settings", { data: { ttsDailyCharLimit: 0, dailyNewLimit: 20 } });
  await page.request.post("/api/import/cards", {
    multipart: {
      deck: "mobile",
      file: {
        name: "mobile.csv",
        mimeType: "text/csv",
        buffer: Buffer.from("word,meaning\naccept,接受\nreply,回答\ncreate,创造"),
      },
    },
  });
  const cards = await (await page.request.get("/api/cards?deck=mobile")).json();
  for (const card of cards.rows)
    await page.request.post("/api/review/rate", {
      data: { cardId: card.id, rating: "easy", durationMs: 10 },
    });
  const result = await page.request.post("/api/articles", { data: { genre: "dialogue" } });
  expect(result.status()).toBe(200);
  const { articleId } = await result.json();
  await page.goto(`/article/${articleId}`);
  await expect(page.getByRole("button", { name: "显示原文" })).toBeVisible();
  await page.getByRole("button", { name: "生成音频", exact: true }).click();
  await expect(page.locator(".notice.error")).toContainText("上限 0");
  await page.getByRole("button", { name: "改用浏览器朗读", exact: true }).last().click();
  await page.getByLabel("播放速度").selectOption(".75".replace(/^\./, "0."));
  await page.getByRole("button", { name: "播放", exact: true }).click();
  await expect
    .poll(() => page.evaluate(() => (window as unknown as { spokenRate: number }).spokenRate))
    .toBe(0.75);
  await expect(page.getByText("听完后，感觉如何？")).toBeVisible();
  await page.getByRole("button", { name: "显示原文" }).click();
  const view = await (await page.request.get(`/api/articles/${articleId}`)).json();
  expect(await page.locator(".body-text").textContent()).toBe(view.body);
  const cold = page
    .locator("[data-start]")
    .filter({ hasText: /^cold$/ })
    .first();
  await cold.focus();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("dialog")).toContainText("语境释义（测试）");
  await page.getByRole("button", { name: "认识", exact: true }).click();
  await expect(page.getByRole("dialog").getByRole("status")).toContainText("已标记");
  await page.getByRole("button", { name: "加入复习", exact: true }).click();
  await expect(page.getByRole("dialog").getByRole("status")).toContainText(/已在复习中|已加入复习/);
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).not.toBeVisible();
  await page
    .locator("[data-start]")
    .filter({ hasText: /^weekend$/ })
    .first()
    .click();
  await page.getByRole("button", { name: "标记不认识", exact: true }).click();
  await expect(page.getByRole("dialog").getByRole("status")).toContainText("已标记");
  await page.keyboard.press("Escape");
  await page.locator(".reading-end").scrollIntoViewIfNeeded();
  await expect(page.getByRole("button", { name: "已读完 ✓", exact: true })).toBeDisabled({
    timeout: 10000,
  });
  await page.goto("/inbox");
  await page.getByLabel("全选").check();
  await page.getByRole("button", { name: "忽略", exact: true }).click();
  await expect(page.getByText("没有待处理的生词")).toBeVisible();
  for (const path of ["/today", "/review", "/library", "/inbox", "/settings", "/stats"]) {
    await page.goto(path);
    await expect(page.getByRole("main")).toBeVisible();
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth),
    ).toBe(true);
  }
  expect((await page.request.get("/article/00000000-0000-7000-8000-000000000000")).status()).toBe(
    404,
  );
  expect(
    (await page.request.post("/api/review/rate", { data: { rating: "invalid" } })).status(),
  ).toBe(400);
  expect(
    (
      await page.request.post("/api/import/cards", {
        multipart: {
          deck: "oversize",
          file: {
            name: "too-big.csv",
            mimeType: "text/csv",
            buffer: Buffer.alloc(20 * 1024 * 1024 + 1),
          },
        },
      })
    ).status(),
  ).toBe(413);
  await page.goto("/today");
  await expect(page.locator(".generation-panel")).toBeVisible();
  await page.screenshot({ path: "test-results/mobile-today.png", fullPage: true });
});

test("100-word import, twenty new-card keyboard reviews and daily-limit completion", async ({
  page,
}) => {
  await page.goto("/login");
  await page.getByLabel("访问密码").fill("test-password");
  await page.getByRole("button", { name: "解锁学习空间" }).click();
  await expect(page).toHaveURL(/today/);
  const stats = await (await page.request.get("/api/stats")).json();
  // Shared in-memory server: reserve exactly 20 remaining introductions for this test.
  await page.request.post("/api/settings", {
    data: { dailyNewLimit: stats.learning.today.newLearned + 20 },
  });
  const existing = await (await page.request.get("/api/cards?limit=100")).json();
  const known = new Set(existing.rows.map((card: { lemma: string }) => card.lemma));
  const words = readFileSync("../../fixtures/wordlist/baseline-sample.txt", "utf8")
    .trim()
    .split("\n")
    .filter((word) => !known.has(word.toLowerCase()))
    .slice(0, 100);
  expect(words).toHaveLength(100);
  await page.goto("/library");
  await page.getByLabel("导入文件").setInputFiles({
    name: "hundred.csv",
    mimeType: "text/csv",
    buffer: Buffer.from(`word,meaning\n${words.map((word) => `${word},合成测试释义`).join("\n")}`),
  });
  await page.getByRole("button", { name: "开始导入" }).click();
  await expect(page.getByRole("status")).toContainText("新增 100");
  await page.goto("/review");
  for (let i = 0; i < 20; i++) {
    const heading = page.locator(".flashcard h2");
    await expect(heading).toBeVisible();
    const word = await heading.textContent();
    await page.keyboard.press("Space");
    await expect(page.getByRole("button", { name: /^4 · Easy/ })).toBeVisible();
    await page.keyboard.press("4");
    if (i < 19) await expect(heading).not.toHaveText(word ?? "");
    else await expect(heading).not.toBeVisible();
  }
  await expect(page.getByText("今日已完成", { exact: true })).toBeVisible();
  const after = await (await page.request.get("/api/stats")).json();
  expect(after.learning.today.newLearned).toBe(stats.learning.today.newLearned + 20);
});

test("baseline import and merge show word/card counts; autoAdd mode warns and reports actual card outcome", async ({
  page,
}) => {
  await page.goto("/login");
  await page.getByLabel("访问密码").fill("test-password");
  await page.getByRole("button", { name: "解锁学习空间" }).click();
  await expect(page).toHaveURL(/today/);
  await page.goto("/library");
  await expect(page.getByText(/CSV\/TXT 最大 20 MiB；.apkg 最大 64 MiB/)).toBeVisible();
  await page.getByLabel("导入类型").selectOption("wordlist");
  await page.getByLabel("词表名称").fill("e2e baseline");
  await page.getByLabel("导入文件").setInputFiles({
    name: "baseline.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("acorn\nhazel"),
  });
  await page.getByRole("button", { name: "开始导入" }).click();
  await expect(page.getByRole("status")).toContainText("新增词条 2 · 新增卡片 0");
  // React form actions reset uncontrolled fields after a successful import.
  await page.getByLabel("词表名称").fill("e2e baseline");
  await page.getByLabel("合并同名词表").check();
  await page.getByLabel("导入文件").setInputFiles({
    name: "baseline.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("hazel\nspruce"),
  });
  await page.getByRole("button", { name: "开始导入" }).click();
  await expect(page.getByRole("status")).toContainText("新增词条 1 · 新增卡片 0");
  await expect(page.getByRole("status")).toContainText("重复 1");
  const baselineCards = await (await page.request.get("/api/cards?q=acorn")).json();
  expect(baselineCards.total).toBe(0);
  await page.goto("/settings");
  await page.getByLabel("点词模式").selectOption("autoAdd");
  await page.getByRole("button", { name: "保存设置" }).click();
  await expect(page.getByRole("status")).toContainText("已保存");
  const today = await (await page.request.get("/api/today")).json();
  const articleId = today.articles[0].id;
  await page.goto(`/article/${articleId}`);
  await page.getByRole("button", { name: "显示原文" }).click();
  await page
    .locator("[data-start]")
    .filter({ hasText: /^weekend$/ })
    .first()
    .click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toContainText("当前模式：自动建卡");
  await expect(dialog).toContainText("标记不认识会加入复习，并计入每日新词上限");
  await page.getByRole("button", { name: "标记不认识", exact: true }).click();
  await expect(dialog.getByRole("status")).toContainText("已标记不认识，并加入复习");
  const cards = await (await page.request.get("/api/cards?q=weekend")).json();
  expect(cards.total).toBe(1);
  expect(cards.rows[0].origin).toBe("autoAdd");
  const inbox = await (await page.request.get("/api/inbox")).json();
  expect(inbox.some((row: { lemma: string }) => row.lemma === "weekend")).toBe(false);
});
