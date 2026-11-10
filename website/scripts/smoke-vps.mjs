import assert from 'node:assert/strict';
import { chromium } from '@playwright/test';

const baseURL=process.env.SITE_TEST_URL ?? 'http://127.0.0.1:3045';
const browser=await chromium.launch({headless:true});
try {
  const page=await browser.newPage({viewport:{width:1440,height:1000}});
  const errors=[];
  const failed=[];
  page.on('pageerror',error=>errors.push(error.message));
  page.on('response',r=>{if(r.status()>=400) failed.push(`${r.status()} ${r.url()}`);});
  const response=await page.goto(baseURL,{waitUntil:'networkidle'});
  assert.equal(response.status(),200);
  await page.waitForTimeout(2500);
  assert.match(await page.title(),/cppm/);
  assert.ok(await page.locator('h1').innerText());
  assert.deepEqual(errors,[],'Browser exceptions');
  assert.deepEqual(failed,[],'Failed assets');
  for(const width of [1440,390]) {
    await page.setViewportSize({width,height:1000});
    await page.waitForTimeout(500);
    assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth+2),false,`Horizontal overflow at ${width}px`);
  }
  console.log(`Browser check passed: ${await page.title()}, desktop and mobile, no missing assets`);
} finally { await browser.close(); }
