import { readFileSync } from "node:fs";
import { describe, it, expect } from "vitest";
import {
  SECTIONS,
  ENV_VAR_DEFS,
  ENV_VAR_GROUP_ORDER,
  ENV_VAR_GROUP_LABEL_KEYS,
  ALL_SETTING_KEYS,
} from "./claude-settings-schema";

describe("claude-settings-schema", () => {
  it("每个 settings 键只出现一次", () => {
    const dupes = ALL_SETTING_KEYS.filter((k, i) => ALL_SETTING_KEYS.indexOf(k) !== i);
    expect(dupes).toEqual([]);
  });

  it("每个 section 的 id 唯一且有 labelKey", () => {
    const ids = SECTIONS.map((s) => s.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const s of SECTIONS) expect(s.labelKey).toMatch(/^settings\.section/);
  });

  it("object 字段必须声明 objectFields，kv-select 必须声明 valueOptions", () => {
    for (const f of SECTIONS.flatMap((s) => s.fields)) {
      if (f.type === "object") expect(f.objectFields?.length, f.key).toBeGreaterThan(0);
      if (f.type === "kv-select") expect(f.valueOptions?.length, f.key).toBeGreaterThan(0);
      if (f.type === "select") expect(f.options?.length, f.key).toBeGreaterThan(0);
    }
  });

  it("每个环境变量只出现一次", () => {
    const keys = ENV_VAR_DEFS.map((d) => d.key);
    const dupes = keys.filter((k, i) => keys.indexOf(k) !== i);
    expect(dupes).toEqual([]);
  });

  it("环境变量的 group 都在 ENV_VAR_GROUP_ORDER 内且有 i18n labelKey", () => {
    for (const d of ENV_VAR_DEFS) {
      expect(ENV_VAR_GROUP_ORDER as readonly string[], d.key).toContain(d.group);
      expect(ENV_VAR_GROUP_LABEL_KEYS[d.group], d.group).toBeTruthy();
    }
  });

  it("select 类型的环境变量必须带 options", () => {
    for (const d of ENV_VAR_DEFS.filter((d) => d.type === "select")) {
      expect(d.options?.length, d.key).toBeGreaterThan(0);
    }
  });

  // 上游文档对齐回归护栏：这些键 2026-08 从官方 settings / env-vars 文档补入，
  // 误删会让可视化面板重新出现覆盖缺口。
  it("覆盖官方文档新增的关键 settings 键", () => {
    for (const k of [
      "advisorModel", "availableModels", "fallbackModel", "skillOverrides",
      "skillListingMaxDescChars", "autoCompactEnabled", "fileCheckpointingEnabled",
      "askUserQuestionTimeout", "crossSessionInbound", "theme", "voice",
      "spinnerVerbs", "diffTool", "enableAllProjectMcpServers", "disableRemoteControl",
    ]) {
      expect(ALL_SETTING_KEYS, k).toContain(k);
    }
  });

  it("maxSkillDescriptionChars 已替换为上游的 skillListingMaxDescChars", () => {
    expect(ALL_SETTING_KEYS).not.toContain("maxSkillDescriptionChars");
    expect(ALL_SETTING_KEYS).toContain("skillListingMaxDescChars");
  });

  it("覆盖官方 env-vars 文档的全部 332 个变量（含额外 OTel 标准变量）", () => {
    expect(ENV_VAR_DEFS.length).toBeGreaterThanOrEqual(332);
    for (const k of [
      "MCP_TIMEOUT", "OTEL_LOG_USER_PROMPTS", "CLAUDE_CONFIG_DIR",
      "VERTEX_REGION_CLAUDE_5_OPUS", "CLAUDE_CODE_SYNC_SKILLS", "DO_NOT_TRACK",
    ]) {
      expect(ENV_VAR_DEFS.map((d) => d.key), k).toContain(k);
    }
  });

  // 环境变量走 t(`env.${key}`) 动态模板，check-i18n 的动态检查只提示不判红，
  // 漏译只会在切语言时露出英文/中文回退，所以这里硬校验 8 语言全覆盖。
  const LOCALES = ["zh-Hans", "en-US", "ar-SA", "fr-FR", "de-DE", "ru-RU", "ja-JP", "es-ES"];
  it.each(LOCALES)("%s 覆盖所有环境变量的 label 与 desc", (locale) => {
    const dict = JSON.parse(
      readFileSync(`src-tauri/crates/aidog_i18n/locales/${locale}.json`, "utf8"),
    ) as Record<string, string>;
    const missing = ENV_VAR_DEFS.flatMap((d) =>
      [`env.${d.key}`, `env.${d.key}.desc`].filter((k) => !dict[k]?.trim()),
    );
    expect(missing).toEqual([]);
  });
});

// 内置默认的隐私基线：这些键一旦被删，新用户会在无提示的情况下重新暴露凭证或自动联网。
describe("defaults/settings.json 隐私基线", () => {
  const defaults = JSON.parse(
    readFileSync("src-tauri/defaults/settings.json", "utf8"),
  ) as {
    permissions: { deny: string[]; ask: string[] };
    env: Record<string, string>;
    [k: string]: unknown;
  };

  it("顶层隐私开关已开", () => {
    expect(defaults.disableClaudeAiConnectors).toBe(true);
    expect(defaults.isolatePeerMachines).toBe(true);
  });

  it("env 覆盖凭证隔离与自动联网三项", () => {
    for (const k of [
      "CLAUDE_CODE_MCP_ALLOWLIST_ENV",
      "CLAUDE_CODE_DISABLE_OFFICIAL_MARKETPLACE_AUTOINSTALL",
      "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC",
    ]) {
      expect(defaults.env[k], k).toBe("1");
    }
  });

  // CLAUDE_CODE_SUBPROCESS_ENV_SCRUB=1 会让 Claude Code 强制把权限模式压成 default
  // （"Permission mode forced to default — CLAUDE_CODE_SUBPROCESS_ENV_SCRUB is set"），
  // 连显式 --permission-mode bypassPermissions 都被它覆盖，与下面的 defaultMode 冲突。
  it("不设 CLAUDE_CODE_SUBPROCESS_ENV_SCRUB，且 defaultMode 为 bypassPermissions", () => {
    expect(defaults.env.CLAUDE_CODE_SUBPROCESS_ENV_SCRUB).toBeUndefined();
    expect((defaults.permissions as { defaultMode?: string }).defaultMode).toBe(
      "bypassPermissions",
    );
  });

  it("deny 覆盖云厂商与包管理器凭证路径", () => {
    for (const rule of [
      "Read(~/.aws/**)",
      "Read(~/.config/gcloud/**)",
      "Read(~/.kube/config)",
      "Read(~/.claude/.credentials.json)",
      "Read(**/.git-credentials)",
      "Read(**/.netrc)",
      "Read(**/.npmrc)",
      "Read(**/.pgpass)",
      "Read(**/*.kdbx)",
    ]) {
      expect(defaults.permissions.deny, rule).toContain(rule);
    }
  });

  it("已安装依赖目录禁改", () => {
    for (const dir of [
      "**/node_modules/**", "**/vendor/**", "**/bower_components/**",
      "**/.yarn/**", "**/.pnpm-store/**", "**/.venv/**",
      "**/site-packages/**", "**/Pods/**",
      "~/.cargo/registry/**", "~/go/pkg/mod/**",
    ]) {
      expect(defaults.permissions.deny, dir).toContain(`Edit(${dir})`);
    }
  });

  // Claude Code 的文件权限检查只匹配 Edit(path)；Write(path) 规则不生效（且 Edit
  // 规则覆盖包括 Write 在内的全部文件编辑工具）。写成 Write(...) 等于没拦。
  it("不存在 Write(path) 形式的失效规则", () => {
    const dead = [...defaults.permissions.deny, ...defaults.permissions.ask]
      .filter((r) => r.startsWith("Write("));
    expect(dead).toEqual([]);
  });

  it("锁文件禁改、依赖清单需确认", () => {
    const LOCKFILES = [
      "go.sum", "yarn.lock", "package-lock.json", "pnpm-lock.yaml", "Cargo.lock",
      "composer.lock", "Podfile.lock", "poetry.lock", "uv.lock", "bun.lockb",
      "bun.lock", "deno.lock", "flake.lock", "gradle.lockfile",
      "packages.lock.json", "mix.lock", ".terraform.lock.hcl",
    ];
    const MANIFESTS = [
      "go.mod", "package.json", "Cargo.toml", "Gemfile", "pyproject.toml",
      "requirements.txt", "Pipfile", "deno.json", "bunfig.toml",
      "composer.json", "Podfile", "mix.exs",
    ];
    for (const [files, list] of [
      [LOCKFILES, defaults.permissions.deny],
      [MANIFESTS, defaults.permissions.ask],
    ] as const) {
      for (const f of files) {
        expect(list, f).toContain(`Edit(**/${f})`);
      }
    }
  });

  it("env 每个键都在 ENV_VAR_DEFS 内（可视化面板可编辑）", () => {
    const known = new Set(ENV_VAR_DEFS.map((d) => d.key));
    expect(Object.keys(defaults.env).filter((k) => !known.has(k))).toEqual([]);
  });
});
