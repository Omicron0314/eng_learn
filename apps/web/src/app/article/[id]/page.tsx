import { notFound } from "next/navigation";
import { z } from "zod";
import { getDeps } from "../../../server/runtime";
import { getArticleView } from "../../../server/services/article";
export const dynamic = "force-dynamic";

import { Article } from "../../../components/Article";
export default async function Page({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  if (!z.string().uuid().safeParse(id).success) notFound();
  if (!getArticleView(getDeps(), id)) notFound();
  return <Article key={id} id={id} />;
}
