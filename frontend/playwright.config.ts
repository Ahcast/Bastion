import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: "e2e",
  workers: 1,
  use: {
    baseURL: "http://localhost:4200",
    headless: true,
    launchOptions: {
      executablePath: process.env["BASTION_BROWSER_EXECUTABLE"] || undefined,
      args: [
        "--no-sandbox",
        ...(process.env["BASTION_BROWSER_ARGS"] || "")
          .split(" ")
          .filter(Boolean),
      ],
    },
  },
  reporter: "list",
  outputDir: "/tmp/bastion-browser-results",
});
