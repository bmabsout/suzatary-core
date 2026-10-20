#!/usr/bin/env node
// TestTailnet's browser: Chromium, whose network stack is the WebView's, loads
// the box through the portal's proxy, answering its Basic challenge as
// Route.answers does, and prints the page. The stand-in box's certificate is
// self-signed, so certificate errors are ignored here; the Go client in the
// test verifies TLS end to end.
//
//   chromium.mjs <proxy host:port> <user> <password> <url>
import { createRequire } from "node:module";
import { execFileSync } from "node:child_process";

const [proxy, username, password, url] = process.argv.slice(2);
const root = execFileSync("npm", ["root", "-g"]).toString().trim();
const { chromium } = createRequire(root + "/")("playwright");

const browser = await chromium.launch({ proxy: { server: `http://${proxy}`, username, password } });
try {
  const page = await (await browser.newContext({ ignoreHTTPSErrors: true })).newPage();
  const response = await page.goto(url);
  console.log(response.status(), await page.textContent("body"));
} finally {
  await browser.close();
}
