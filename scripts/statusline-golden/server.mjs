// Standalone mock /api/group-info server (separate process so the synchronous
// script runner in build.mjs can't starve its event loop). Prints the chosen
// port to stdout, serves the given payload to every request.
import http from "node:http";
import { readFileSync } from "node:fs";
const body = readFileSync(process.argv[2]);
const srv = http.createServer((req, res) => {
  req.resume();
  res.setHeader("content-type", "application/json");
  res.end(body);
});
srv.listen(0, "127.0.0.1", () => {
  process.stdout.write(String(srv.address().port) + "\n");
});
