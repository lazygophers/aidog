import { chromium } from 'playwright';
const url = 'file://' + process.cwd() + '/Home.html';
const combos = [
  { style: '毛玻璃', palette: 'Nord', mode: 'light', out: 'shot-1-glass-nord-light.png' },
  { style: '便当', palette: 'Apple Blue', mode: '深色', out: 'shot-2-bento-apple-dark.png' },
  { style: '纸感', palette: 'Gruvbox', mode: '浅色', out: 'shot-3-paper-gruvbox-light.png' },
  { style: '终端', palette: 'Dracula', mode: '深色', out: 'shot-4-terminal-dracula-dark.png' },
];
const b = await chromium.launch();
const pg = await b.newPage({ viewport: { width: 1280, height: 1180 } });
await pg.goto(url, { waitUntil: 'networkidle' });
async function clickPill(text) {
  const el = pg.locator('.pill', { hasText: new RegExp('^' + text + '$') }).first();
  await el.click();
  await pg.waitForTimeout(250);
}
for (const c of combos) {
  await clickPill(c.mode);
  await clickPill(c.style);
  await clickPill(c.palette);
  await pg.waitForTimeout(350);
  await pg.screenshot({ path: c.out });
  console.log('shot', c.out);
}
await b.close();
