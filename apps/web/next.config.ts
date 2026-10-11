import path from "node:path";
import type { NextConfig } from "next";
import { PROXY_BODY_MAX_BYTES } from "./src/import-limits";

const config: NextConfig = {
  output: "standalone",
  experimental: { proxyClientMaxBodySize: PROXY_BODY_MAX_BYTES },
  outputFileTracingRoot: path.join(process.cwd(), "../.."),
  transpilePackages: ["@eng-learn/core", "@eng-learn/db", "@eng-learn/providers"],
  serverExternalPackages: ["better-sqlite3"],
  outputFileTracingIncludes: { "/*": ["../../prompts/**/*"] },
};
export default config;
