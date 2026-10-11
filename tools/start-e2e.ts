import { cpSync, existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

// CI builds first. Serve exactly the standalone output used by Docker, including
// browser assets that Next does not copy into the deployment automatically.
const web = fileURLToPath(new URL("../apps/web/", import.meta.url));
const deployment = path.join(web, ".next/standalone/apps/web");
const server = path.join(deployment, "server.js");
if (!existsSync(server)) throw new Error("Run pnpm build before CI E2E");
cpSync(path.join(web, ".next/static"), path.join(deployment, ".next/static"), {
  recursive: true,
});
cpSync(path.join(web, "public"), path.join(deployment, "public"), { recursive: true });
process.env.PORT = "3100";
process.env.HOSTNAME = "127.0.0.1";
await import(pathToFileURL(server).href);
