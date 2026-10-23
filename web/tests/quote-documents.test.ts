import { test } from 'node:test';
import assert from 'node:assert/strict';
import { hondurasSchedule, pdfBlob, reservePdfViewer } from '../src/quoteDocuments.js';
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
import { fileBlob } from '../src/quoteAttachments.js';

test('PDF reserves a browser tab, removes its opener and shows a blob URL without a download link',()=>{
  const previous=Object.getOwnPropertyDescriptor(globalThis,'window');
  const target={closed:false,opener:{} as unknown,document:{title:'',body:{textContent:''}},location:{href:'about:blank',replace(value:string){this.href=value;}},close(){this.closed=true;}};
  Object.defineProperty(globalThis,'window',{configurable:true,value:{open:()=>target}});
  let viewer:ReturnType<typeof reservePdfViewer>|undefined;
  try{
    viewer=reservePdfViewer();assert.equal(target.opener,null);assert.equal(target.location.href,'about:blank');
    viewer.show({filename:'quote.pdf',mime:'application/pdf',content_base64:btoa('%PDF-synthetic')});
    assert.ok(target.location.href.startsWith('blob:'));assert.equal(target.document.title,'quote.pdf');
  }finally{viewer?.close();assert.equal(target.closed,true);if(previous)Object.defineProperty(globalThis,'window',previous);else Reflect.deleteProperty(globalThis,'window');}
});
test('blocked PDF tabs report an error and attachment HTML stays a binary download',()=>{
  const previous=Object.getOwnPropertyDescriptor(globalThis,'window');
  Object.defineProperty(globalThis,'window',{configurable:true,value:{open:()=>null}});
  try{assert.throws(()=>reservePdfViewer(),/bloqueó/);}finally{if(previous)Object.defineProperty(globalThis,'window',previous);else Reflect.deleteProperty(globalThis,'window');}
  assert.equal(fileBlob({filename:'a.html',mime:'text/html',content_base64:btoa('<script>bad()</script>')}).type,'application/octet-stream');
  assert.throws(()=>fileBlob({filename:'a.pdf',mime:'application/pdf',content_base64:btoa('<script>bad()</script>')}));
});
