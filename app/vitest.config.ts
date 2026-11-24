import path from "node:path";
import { defineConfig } from "vitest/config";

const alias = { "@": path.resolve(import.meta.dirname, "src") };

// .test.ts files run in node; .test.tsx files get a DOM (happy-dom).
export default defineConfig({
  test: {
    projects: [
      {
        resolve: { alias },
        test: { name: "node", environment: "node", include: ["src/**/*.test.ts"] },
      },
      {
        resolve: { alias },
        test: { name: "dom", environment: "happy-dom", include: ["src/**/*.test.tsx"] },
      },
    ],
  },
});
