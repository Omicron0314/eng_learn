import { type AppDeps, createAppDeps } from "./deps";

const globalDeps = globalThis as typeof globalThis & { appDeps?: AppDeps };
export function getDeps() {
  globalDeps.appDeps ??= createAppDeps(process.env);
  return globalDeps.appDeps;
}
