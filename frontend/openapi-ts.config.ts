import { defineConfig } from "@hey-api/openapi-ts";

export default defineConfig({
  input: "../docs/openapi.json",
  output: "src/lib/api/gen",
  plugins: ["@hey-api/typescript", "zod"],
});
