import path from "node:path";
import type { NextConfig } from "next";

// Next.js buffers proxied request bodies up to this size. Imports have no product
// size limit, so this is only a generous transport buffer rather than a cap.
const UPLOAD_BODY_BUFFER_BYTES = 1024 * 1024 * 1024;

const config: NextConfig = {
  output: "standalone",
  experimental: { proxyClientMaxBodySize: UPLOAD_BODY_BUFFER_BYTES },
  outputFileTracingRoot: path.join(process.cwd(), "../.."),
  transpilePackages: ["@eng-learn/core", "@eng-learn/db", "@eng-learn/providers"],
  serverExternalPackages: ["better-sqlite3"],
  outputFileTracingIncludes: { "/*": ["../../prompts/**/*"] },
};
export default config;
