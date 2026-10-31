import {test,expect} from '@playwright/test';
async function ready(page){await page.goto('/');await page.waitForFunction(()=>window['mdpdf']?.ready);}
async function setSource(page,text){await page.evaluate(text=>{const e=window['mdpdf'].editor;e.dispatch({changes:{from:0,to:e.state.doc.length,insert:text},selection:{anchor:0}});},text);}
async function command(page,name){await page.keyboard.press('ControlOrMeta+Shift+p');await page.getByLabel('Search commands').fill(name);await page.getByLabel('Search commands').press('Enter');}
test('palette formats selected Markdown, undo works, and outline navigates headings',async({page})=>{
 await ready(page);await setSource(page,'# Title\n\n## Details\n\nhello');
 await page.evaluate(()=>{const e=window['mdpdf'].editor;e.dispatch({selection:{anchor:e.state.doc.length-5,head:e.state.doc.length}});e.focus();});
 await page.keyboard.press('ControlOrMeta+b');await expect(page.locator('#editor')).toContainText('**hello**');
 await page.keyboard.press('ControlOrMeta+z');await expect.poll(()=>page.evaluate(()=>window['mdpdf'].editor.state.doc.toString())).toBe('# Title\n\n## Details\n\nhello');
 await command(page,'Document outline');await page.getByRole('button',{name:'Details',exact:true}).click();
 expect(await page.evaluate(()=>window['mdpdf'].editor.state.doc.lineAt(window['mdpdf'].editor.state.selection.main.head).number)).toBe(3);
 await page.locator('#read-mode').click();await expect(page.locator('#live-preview')).toBeFocused();
 await page.keyboard.press('ControlOrMeta+Shift+p');await expect(page.getByRole('dialog',{name:'Commands',exact:true})).toBeVisible();
});
test('table grid edits columns and alignment and smart paste converts TSV and links',async({page})=>{
 await ready(page);await setSource(page,'');await command(page,'Insert or edit table');
 await page.getByLabel('Row 1 column 1').fill('Name');await page.getByLabel('Row 2 column 1').fill('Alice');await page.getByLabel('Column 1 alignment').selectOption('center');await page.getByRole('button',{name:'Add column',exact:true}).click();await page.getByRole('button',{name:'Apply table',exact:true}).click();
 await expect(page.locator('#editor')).toContainText('| Name | Column 2 |');await expect(page.locator('#editor')).toContainText(':---:');
 await setSource(page,'Selected');await page.evaluate(()=>{const e=window['mdpdf'].editor;e.dispatch({selection:{anchor:0,head:8}});const dt=new DataTransfer();dt.setData('text/plain','https://example.com');document.querySelector('#editor .cm-content').dispatchEvent(new ClipboardEvent('paste',{clipboardData:dt,bubbles:true,cancelable:true}));});
 await expect(page.locator('#editor')).toContainText('[Selected](https://example.com)');
 await setSource(page,'');await page.evaluate(()=>{const dt=new DataTransfer();dt.setData('text/plain','Name\tCost\nAlice\t10');document.querySelector('#editor .cm-content').dispatchEvent(new ClipboardEvent('paste',{clipboardData:dt,bubbles:true,cancelable:true}));});
 await expect(page.locator('#editor')).toContainText('| Name | Cost |');await expect(page.frameLocator('#live-preview').locator('table')).toBeVisible();
});
test('library keeps documents and restores history across reloads',async({page})=>{
 await ready(page);await setSource(page,'# Original');await page.locator('#filename').fill('First.md');
 await page.getByRole('button',{name:'Documents',exact:true}).click();await expect(page.getByRole('button',{name:'● First.md'})).toBeVisible();await page.getByRole('button',{name:'New document',exact:true}).click();
 await expect.poll(()=>page.evaluate(()=>window['mdpdf'].editor.state.doc.toString())).toBe('');await setSource(page,'# Second');
 await page.getByRole('button',{name:'Documents',exact:true}).click();await page.getByRole('button',{name:'First.md',exact:true}).click();await expect(page.locator('#editor')).toContainText('# Original');
 await setSource(page,'# Changed');await page.getByRole('button',{name:'Documents',exact:true}).click();await page.getByRole('button',{name:'● First.md'}).locator('..').getByRole('button',{name:'History'}).click();
 await expect(page.getByRole('dialog',{name:'Document history'})).toContainText('# Original');await page.getByRole('dialog',{name:'Document history'}).locator('summary').first().click();await page.getByRole('button',{name:'Restore this version'}).first().click();await expect(page.locator('#editor')).toContainText('# Original');
 await page.reload();await page.waitForFunction(()=>window['mdpdf']?.ready);await expect(page.locator('#editor')).toContainText('# Original');
});
test('local images survive a new server session and mobile tools stay reachable',async({page,context})=>{
 await page.setViewportSize({width:390,height:850});await ready(page);await setSource(page,'# Images\n\n');await page.evaluate(()=>{const e=window['mdpdf'].editor;e.dispatch({selection:{anchor:e.state.doc.length}});});
 await page.locator('#image-file').setInputFiles('e2e/fixtures/pixel.png');
 await page.locator('[data-pane=preview]').click();
 await expect.poll(()=>page.frameLocator('#live-preview').locator('img').evaluate((img:HTMLImageElement)=>img.naturalWidth)).toBe(1);
 await page.getByRole('button',{name:'Documents',exact:true}).click();await expect(page.getByRole('dialog',{name:'Local documents'})).toBeInViewport();await page.getByRole('dialog',{name:'Local documents'}).getByRole('button',{name:'Close',exact:true}).click();
 await page.getByRole('button',{name:'Outline',exact:true}).click();await page.getByRole('button',{name:'Images',exact:true}).click();await expect(page.locator('#workspace')).toHaveAttribute('data-mobile-pane','editor');
 await context.clearCookies();await page.reload();await page.waitForFunction(()=>window['mdpdf']?.ready);await page.locator('[data-pane=preview]').click();await expect.poll(()=>page.frameLocator('#live-preview').locator('img').evaluate((img:HTMLImageElement)=>img.naturalWidth)).toBe(1);
});
