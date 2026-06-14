// Build golden fixtures: for each (config, input) pair, generate the script
// (bash today, python after conversion), run it on the fixture stdin, capture
// stdout byte-for-byte. Group-* configs use a local mock /api/group-info server.
//
// Usage:
//   node build.mjs golden        → write *.golden (run current generator output)
//   node build.mjs check         → run generator, diff against *.golden (exit 1 on mismatch)
import { execFileSync, spawn } from "node:child_process";
import { mkdirSync, writeFileSync, readFileSync, existsSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = join(HERE, "../..");
const INPUTS = join(HERE, "inputs");
const GOLDEN = join(HERE, "golden");
const TMP = join(HERE, ".tmp");
mkdirSync(GOLDEN, { recursive: true });
mkdirSync(TMP, { recursive: true });

const mode = process.argv[2] || "check";
// `golden --bash` regenerates the golden from the *original bash* generators
// (.bash-orig.ts, extracted from git HEAD). Plain `golden` / `check` use the
// current (Python) generator. Authentic golden must always be `golden --bash`.
const useBash = process.argv.includes("--bash");

// (config, kind, input, [groupFixture]) cases. groupFixture → serve via mock server.
const CASES = [
  ["main-default", "main", "full"],
  ["main-default", "main", "minimal"],
  ["main-default", "main", "zeros"],
  ["main-default", "main", "huge"],
  ["main-default", "main", "rtl"],
  ["atoms-fixed", "main", "full"],
  ["atoms-fixed", "main", "minimal"],
  ["atoms-fixed", "main", "zeros"],
  ["atoms-fixed", "main", "huge"],
  ["atoms-fixed", "main", "edges"],
  ["atoms-fields", "main", "full"],
  ["atoms-fields", "main", "minimal"],
  ["atoms-fields", "main", "rtl"],
  ["atoms-autocolor", "main", "full"],
  ["atoms-autocolor", "main", "zeros"],
  ["atoms-autocolor", "main", "huge"],
  ["align-rows", "main", "full"],
  ["align-rows", "main", "minimal"],
  // group: main-default row 3 + dedicated group configs, served payloads
  ["main-default", "main", "full", "groupinfo"],
  ["main-default", "main", "full", "groupinfo_red"],
  ["group-static", "main", "full", "groupinfo"],
  ["group-static", "main", "full", "groupinfo_na"],
  ["group-dynamic", "main", "full", "groupinfo"],
  ["group-dynamic", "main", "full", "groupinfo_red"],
  // subagent
  ["subagent-default", "subagent", "sa_multi"],
  ["subagent-default", "subagent", "sa_empty"],
  ["subagent-default", "subagent", "sa_notasks"],
  ["subagent-default", "subagent", "sa_noid"],
  ["subagent-default", "subagent", "sa_times"],
];

// Bundle the chosen emitter once (esbuild from node_modules).
const esbuild = join(ROOT, "node_modules/.bin/esbuild");
const emitBundle = join(TMP, "emit.bundle.mjs");
const emitSrc = useBash ? "emit-bash.mjs" : "emit.mjs";
execFileSync(esbuild, [
  join(HERE, emitSrc), "--bundle", "--platform=node", "--format=esm",
  `--outfile=${emitBundle}`,
], { stdio: ["ignore", "ignore", "inherit"] });

function genScript(cfg, kind) {
  return execFileSync("node", [emitBundle, kind, cfg]);
}

function startServer(payloadPath) {
  return new Promise((resolve, reject) => {
    // Separate process: the synchronous script runner (execFileSync) below would
    // otherwise starve an in-process server's event loop.
    const child = spawn("node", [join(HERE, "server.mjs"), payloadPath], {
      stdio: ["ignore", "pipe", "inherit"],
    });
    let buf = "";
    child.stdout.on("data", (d) => {
      buf += d.toString();
      const nl = buf.indexOf("\n");
      if (nl >= 0) resolve({ child, port: parseInt(buf.slice(0, nl), 10) });
    });
    child.on("error", reject);
  });
}

const isPy = (buf) => buf.slice(0, 32).toString().includes("python");
const ext = (buf) => (isPy(buf) ? "py" : "sh");

let fails = 0, total = 0;
for (const [cfg, kind, input, groupFx] of CASES) {
  total++;
  const scriptBuf = genScript(cfg, kind);
  const scriptPath = join(TMP, `${cfg}__${kind}.${ext(scriptBuf)}`);
  writeFileSync(scriptPath, scriptBuf, { mode: 0o755 });
  const stdin = readFileSync(join(INPUTS, `${input}.json`));

  // Prepend the deterministic `date` shim so `date +%s` returns a fixed epoch
  // (group coding-plan reset deltas + subagent startTime elapsed stay stable).
  const shimDir = join(HERE, "shim");
  const env = {
    ...process.env,
    LC_ALL: "C.UTF-8", LANG: "C.UTF-8", TZ: "UTC",
    PATH: `${shimDir}:${process.env.PATH}`,
  };
  let server = null;
  if (groupFx) {
    server = await startServer(join(INPUTS, `${groupFx}.json`));
    env.ANTHROPIC_BASE_URL = `http://127.0.0.1:${server.port}/v1`;
    env.ANTHROPIC_AUTH_TOKEN = "test-group";
  } else {
    delete env.ANTHROPIC_BASE_URL;
  }

  const runner = isPy(scriptBuf) ? "python3" : "bash";
  let stdout;
  try {
    // Run from a non-git cwd so a `git -C .` fallback (input without
    // workspace.current_dir) degrades to empty deterministically.
    stdout = execFileSync(runner, [scriptPath], {
      input: stdin, env, maxBuffer: 1 << 24, cwd: "/",
    });
  } catch (e) {
    stdout = e.stdout ?? Buffer.from("");
  }
  if (server) server.child.kill();

  const name = `${cfg}__${kind}__${input}${groupFx ? "__" + groupFx : ""}.golden`;
  const goldenPath = join(GOLDEN, name);
  if (mode === "golden") {
    writeFileSync(goldenPath, stdout);
    console.log("WROTE", name, `(${stdout.length}B)`);
  } else {
    if (!existsSync(goldenPath)) { console.error("MISSING golden", name); fails++; continue; }
    const want = readFileSync(goldenPath);
    if (Buffer.compare(want, stdout) !== 0) {
      fails++;
      console.error(`MISMATCH ${name}\n  golden(${want.length}B): ${JSON.stringify(want.toString())}\n  actual(${stdout.length}B): ${JSON.stringify(stdout.toString())}`);
    } else {
      console.log("OK", name, `(${stdout.length}B)`);
    }
  }
}
console.log(`\n${mode}: ${total - fails}/${total} pass`);
process.exit(fails ? 1 : 0);
