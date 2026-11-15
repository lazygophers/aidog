import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';

const script = join(import.meta.dirname, 'check-holidays.mjs');
const locales = ['zh-Hans', 'en-US', 'ar-SA', 'fr-FR', 'de-DE', 'ru-RU', 'ja-JP', 'es-ES'];
const name = Object.fromEntries(locales.map(locale => [locale, 'Calendar']));

function runCalendarCheck(calendar) {
  const dir = mkdtempSync(join(tmpdir(), 'aidog-holidays-'));
  try {
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, 'XX.json'), JSON.stringify(calendar));
    return spawnSync(process.execPath, [script], {
      encoding: 'utf8',
      env: { ...process.env, HOLIDAYS_DIR: dir },
    });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

const validCalendar = {
  code: 'XX',
  name,
  source_urls: ['https://example.org/holidays'],
  years: {
    2026: { holidays: ['2026-01-01'], workdays: [] },
    2027: { holidays: ['2027-01-01'], workdays: [] },
  },
};

test('valid calendar passes with required locales and year coverage', () => {
  const result = runCalendarCheck(validCalendar);
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /1 calendars, 0 errors, 0 warnings/);
});

test('legacy plural names field fails lint', () => {
  const { name, ...calendar } = validCalendar;
  const result = runCalendarCheck({ ...calendar, names: name });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /name must contain exactly all 8 locales/);
});

test('duplicate dates and holiday/workday overlap fail lint', () => {
  const result = runCalendarCheck({
    ...validCalendar,
    years: {
      ...validCalendar.years,
      2026: { holidays: ['2026-01-01', '2026-01-01'], workdays: ['2026-01-01'] },
    },
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /duplicate dates/);
  assert.match(result.stderr, /appears in holidays and workdays/);
});
