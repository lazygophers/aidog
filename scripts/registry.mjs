// registry（src-tauri/defaults/registry/）的 JS 读取层：合并 platforms/*/platform.json
// → 旧 platform-presets.json 等价文档。与 Rust 侧 aidog_db::registry::presets() 同形状。
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const REGISTRY = new URL('../src-tauri/defaults/registry/', import.meta.url).pathname;

/** @returns {{version: string, last_updated: number, protocols: Record<string, any>}} */
export function readPresets() {
  const index = JSON.parse(readFileSync(join(REGISTRY, 'index.json'), 'utf8'));
  const protocols = Object.fromEntries(
    index.platforms.map((p) => [p.code, JSON.parse(readFileSync(join(REGISTRY, p.platform_file), 'utf8'))]),
  );
  return { version: index.version, last_updated: index.last_updated, protocols };
}
