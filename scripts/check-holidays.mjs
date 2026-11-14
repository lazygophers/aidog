import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

const root = process.env.HOLIDAYS_DIR ?? join(import.meta.dirname, '..', 'src-tauri', 'defaults', 'holidays');
const files = [];

function walk(dir) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) walk(path);
    else if (name.endsWith('.json')) files.push(path);
  }
}

if (!existsSync(root)) {
  console.error(`Missing holiday calendar directory: ${root}`);
  process.exit(1);
}
walk(root);
if (files.length === 0) {
  console.error(`No holiday calendar files found: ${root}`);
  process.exit(1);
}
const calendars = new Map();
let errors = 0;
let warnings = 0;
const today = new Date().toISOString().slice(0, 10);
const currentYear = Number(today.slice(0, 4));

function error(path, message) {
  console.error(`${path}: ${message}`);
  errors++;
}

for (const path of files) {
  let calendar;
  try {
    calendar = JSON.parse(readFileSync(path, 'utf8'));
  } catch (e) {
    error(path, `invalid JSON: ${e.message}`);
    continue;
  }
  const code = path.slice(root.length + 1, -5).replaceAll('\\', '/');
  if (calendar.code !== code) error(path, `code must equal filename (${code})`);
  if (!calendar.name || Object.keys(calendar.name).sort().join(',') !== 'ar-SA,de-DE,en-US,es-ES,fr-FR,ja-JP,ru-RU,zh-Hans') {
    error(path, 'name must contain exactly all 8 locales');
  } else if (Object.values(calendar.name).some(name => typeof name !== 'string' || !name.trim())) {
    error(path, 'all locale names must be non-empty strings');
  }
  if (!Array.isArray(calendar.source_urls) || !calendar.source_urls.length || calendar.source_urls.some(url => typeof url !== 'string' || !/^https:\/\//.test(url))) {
    error(path, 'source_urls must contain official HTTPS URLs');
  }
  if (calendar.extends !== undefined && typeof calendar.extends !== 'string') error(path, 'extends must be a calendar code');
  if (!calendar.years || typeof calendar.years !== 'object' || Array.isArray(calendar.years)) {
    error(path, 'years must be an object');
    continue;
  }
  calendars.set(code, calendar);
  for (const [year, dates] of Object.entries(calendar.years)) {
    if (!/^\d{4}$/.test(year)) error(path, `invalid year key: ${year}`);
    if (!dates || !Array.isArray(dates.holidays) || !Array.isArray(dates.workdays)) {
      error(path, `${year} must contain holidays and workdays arrays`);
      continue;
    }
    for (const [field, entries] of Object.entries(dates)) {
      if (field !== 'holidays' && field !== 'workdays') error(path, `${year} has unknown field ${field}`);
      if (new Set(entries).size !== entries.length) error(path, `${year}.${field} has duplicate dates`);
      for (const date of entries) {
        if (typeof date !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(date) || Number(date.slice(0, 4)) !== Number(year) || new Date(`${date}T00:00:00Z`).toISOString().slice(0, 10) !== date) {
          error(path, `${year}.${field} has invalid date ${date}`);
        }
      }
    }
    const workdays = new Set(dates.workdays);
    for (const date of dates.holidays) if (workdays.has(date)) error(path, `${year} date appears in holidays and workdays: ${date}`);
  }
  if (!calendar.years[String(currentYear)]) error(path, `missing current year ${currentYear}`);
  if (!calendar.years[String(currentYear + 1)]) {
    console.warn(`${path}: missing next year ${currentYear + 1}`);
    warnings++;
  }
}

for (const [code, calendar] of calendars) {
  if (calendar.extends && !calendars.has(calendar.extends)) error(join(root, `${code}.json`), `extends missing calendar ${calendar.extends}`);
  const seen = new Set([code]);
  let parent = calendar.extends;
  while (parent && calendars.has(parent)) {
    if (seen.has(parent)) {
      error(join(root, `${code}.json`), `extends cycle through ${parent}`);
      break;
    }
    seen.add(parent);
    parent = calendars.get(parent).extends;
  }
}

console.log(`check-holidays: ${files.length} calendars, ${errors} errors, ${warnings} warnings`);
process.exit(errors ? 1 : 0);
