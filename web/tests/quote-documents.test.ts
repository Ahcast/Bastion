import { test } from 'node:test';
import assert from 'node:assert/strict';
import { hondurasSchedule, pdfBlob } from '../src/quoteDocuments.js';
test('email scheduling always interprets the selected time in Honduras, independently of the browser timezone',()=>{
  assert.equal(hondurasSchedule('2099-10-22T13:00'),'2099-10-22T19:00:00.000Z');
  assert.equal(hondurasSchedule(''),null);
  assert.throws(()=>hondurasSchedule('not a date'));
});
test('a PDF download accepts only the generated PDF content and expected MIME',async()=>{
  const pdf={filename:'quote.pdf',mime:'application/pdf' as const,content_base64:btoa('%PDF-synthetic')};
  assert.equal(await pdfBlob(pdf).text(),'%PDF-synthetic');
  assert.throws(()=>pdfBlob({...pdf,content_base64:btoa('<script>bad</script>')}));
});
