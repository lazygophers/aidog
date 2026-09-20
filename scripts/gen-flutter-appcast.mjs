#!/usr/bin/env node
// 生成 Flutter 壳的 Sparkle/WinSparkle appcast（票 I13）。
//
// 输入：`--dir <目录>` 里放两个签名元数据 JSON（由 release.yml 的 macos/windows job 产出，
// actions/download-artifact 解包后落在这里）+ 同目录下的安装产物本体：
//   mac.json: { "edSignature": "...", "file": "AiDog-Flutter-v0.1.17-macos-universal.zip" }
//   win.json: { "dsaSignature": "...", "file": "AiDog-Flutter-v0.1.17-x64-setup.exe" }
// 输出：`--out <路径>` 的 appcast XML（默认 <dir>/flutter-appcast.xml）。
//
// 共存铁律：本文件只描述 Flutter 产物；老 Tauri 壳读 latest.json，互不相认。
//
// 用法:
//   node scripts/gen-flutter-appcast.mjs --version 0.1.17 --build 42 --dir dist --tag v0.1.17

import { readFileSync, writeFileSync, statSync } from "node:fs";
import { join } from "node:path";

function arg(name, fallback) {
  const i = process.argv.indexOf(`--${name}`);
  return i === -1 ? fallback : process.argv[i + 1];
}

const version = arg("version");
const build = arg("build");
const dir = arg("dir", "dist");
const tag = arg("tag", `v${version}`);
const repo = arg("repo", "lazygophers/aidog");
const out = arg("out", join(dir, "flutter-appcast.xml"));

if (!version || !build) {
  console.error("[gen-flutter-appcast] --version 与 --build 必填");
  process.exit(1);
}

const url = (file) => `https://github.com/${repo}/releases/download/${tag}/${file}`;
const length = (file) => statSync(join(dir, file)).size;

function loadMeta(name) {
  try {
    return JSON.parse(readFileSync(join(dir, name), "utf8"));
  } catch {
    return null;
  }
}

const mac = loadMeta("mac.json");
const win = loadMeta("win.json");

// sparkle:version 用整数 build number（CFBundleVersion = CI 的 GITHUB_RUN_NUMBER）：
// Sparkle 按 tuple 比较版本，纯数字最不容易出幺蛾子；semver 展示在 shortVersionString。
function item({ meta, sigAttr, os, type }) {
  const sig = meta[sigAttr === "sparkle:edSignature" ? "edSignature" : "dsaSignature"];
  if (!sig) throw new Error(`${sigAttr} 缺失`);
  return `    <item>
      <title>AiDog (Flutter) v${version}</title>
      <pubDate>${new Date().toUTCString()}</pubDate>
      <sparkle:version>${build}</sparkle:version>
      <sparkle:shortVersionString>${version}</sparkle:shortVersionString>
      <enclosure
        url="${url(meta.file)}"
        length="${length(meta.file)}"
        type="${type}"
        ${sigAttr}="${sig}"
        sparkle:os="${os}"
      />
    </item>`;
}

const items = [];
if (mac) items.push(item({ meta: mac, sigAttr: "sparkle:edSignature", os: "macos", type: "application/octet-stream" }));
if (win) items.push(item({ meta: win, sigAttr: "sparkle:dsaSignature", os: "windows", type: "application/octet-stream" }));
if (items.length === 0) {
  console.error("[gen-flutter-appcast] mac.json / win.json 一个都没有，无法生成 appcast");
  process.exit(1);
}

const xml = `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle" xmlns:dc="http://purl.org/dc/elements/1.1/">
  <channel>
    <title>AiDog Flutter</title>
${items.join("\n")}
  </channel>
</rss>
`;

writeFileSync(out, xml);
console.log(`[gen-flutter-appcast] ✓ ${out}（${items.length} 个条目）`);
