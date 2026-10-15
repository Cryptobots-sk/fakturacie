import assert from 'node:assert/strict';
import {writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import http from 'node:http';
import {supplier} from './fixtures.mjs';
const base=process.env.TEST_URL || 'http://127.0.0.1:8874';
if (!base.endsWith(':8874')) throw new Error('HTTP tests require the isolated test server on port 8874.');
let cookies=new Map(), token, count=0;
async function request(path,options={}) {
 const response=await fetch(base+path,{...options,headers:{Cookie:[...cookies].map(([k,v])=>`${k}=${v}`).join('; '),...options.headers}});
 for(const cookie of response.headers.getSetCookie()) {const [pair]=cookie.split(';'); const i=pair.indexOf('='); cookies.set(pair.slice(0,i),pair.slice(i+1));}
 return response;
}
const check=(condition,name)=>{assert.ok(condition,name);console.log('PASS '+name);count++;};
async function post(action,data,extra={}) {const response=await request('/api/'+action,{method:'POST',headers:{'Content-Type':'application/json','X-CSRF-Token':token,Origin:base,...extra},body:JSON.stringify(data)});return {status:response.status,...await response.json()};}
// Native fetch can replace Host; use a raw HTTP request for this boundary check.
async function hostStatus(headers) {return new Promise((resolve,reject)=>{const req=http.request(base,{headers},res=>{res.resume();resolve(res.statusCode);});req.on('error',reject);req.end();});}
check(await hostStatus({Host:'evil.example'})===403,'Untrusted Host rejected');
check(await hostStatus({Host:'evil.example','X-Forwarded-Host':'localhost'})===403,'Forwarded Host cannot bypass restrictions');
check(await hostStatus({Host:'faktura.local:8874'})===200,'Local invoice domain accepted');
check(await hostStatus({Host:'FAKTURA.LOCAL:8874'})===200,'Hostname matching is case insensitive');
for(const path of ['/data/fakturacie.sqlite','/config/common.neon','/vendor/autoload.php','/docker/database.php','/router.php']) {
 check([403,404].includes((await request(path)).status),`Private resource inaccessible: ${path}`);
}
const settingsHtml=await(await request('/nastavenia')).text();
token=settingsHtml.match(/name="csrf-token" content="([^"]+)"/)[1];
const settingsBoot=JSON.parse(settingsHtml.match(/id="app-data">([\s\S]*?)<\/script>/)[1]);
check((await post('settings-save',{...supplier,version:settingsBoot.supplier.version})).ok,'Synthetic supplier configured in isolated app');
let boot;
for(const path of ['/','/zakaznici','/nastavenia','/faktury/nova']) {
 const response=await request(path); const html=await response.text(); check(response.status===200 && html.includes('id="app-data"'),`Real Nette page ${path}`);
 token=html.match(/name="csrf-token" content="([^"]+)"/)[1];
 if(path==='/faktury/nova') boot=JSON.parse(html.match(/<script type="application\/json" id="app-data">([\s\S]*?)<\/script>/)[1]);
}
const input={name:'HTTP fixture '+Date.now(),street:'Test 1',zip:'01001',city:'Žilina',country:'Slovensko',paymentDays:14,qrProfile:'epc'};
check((await post('customer-save',input,{'X-CSRF-Token':''})).status===403,'Missing CSRF rejected');
check((await post('customer-save',input,{'Origin':'https://evil.example'})).status===403,'Cross-origin write rejected');
check((await request('/api/customer-save')).status===405,'GET cannot mutate');
const customer=await post('customer-save',input); check(customer.ok,'Customer created over real HTTP');
check((await post('customer-save',{...customer.data,name:'Updated HTTP customer'})).ok,'Customer updated over real HTTP');
check((await post('customer-save',customer.data)).status===409,'Stale customer HTTP edit rejected');
const draft={...boot.invoice,customerId:customer.data.id,items:[{description:'HTTP test <img src=x onerror=alert(1)>',quantity:'2.5',unit:'hod',unitPrice:'42.10',vatRate:23}],qrProfiles:['sk','cz']};
const saved=await post('invoice-save',draft);check(saved.ok && saved.data.number===null,'Draft saved over HTTP');
const stale=await post('invoice-save',draft);check(stale.status===409,'Duplicate HTTP create protected');
const invoice=await post('invoice-issue',{id:saved.data.id,version:saved.data.version});check(invoice.ok && invoice.data.totals.gross===12946,'HTTP issue and decimal arithmetic');
const repeated=await post('invoice-issue',{id:saved.data.id,version:saved.data.version});check(repeated.data.number===invoice.data.number,'Repeated HTTP issue is idempotent');
const pdfResponse=await request(`/faktury/${saved.data.id}/pdf?download=1`); const bytes=Buffer.from(await pdfResponse.arrayBuffer());
check(pdfResponse.status===200 && pdfResponse.headers.get('content-type').includes('application/pdf') && pdfResponse.headers.get('content-disposition').includes('attachment') && bytes.subarray(0,4).toString()==='%PDF','Download returns actual attachment PDF');
await writeFile('temp/verification/http-invoice.pdf',bytes);
const again=Buffer.from(await(await request(`/faktury/${saved.data.id}/pdf`)).arrayBuffer());check(createHash('sha256').update(bytes).digest('hex')===createHash('sha256').update(again).digest('hex'),'Issued PDF bytes stay identical');
const issuedHtml=await(await request(`/faktury/${saved.data.id}`)).text();
check(issuedHtml.includes('Upraviť faktúru') && issuedHtml.includes('id="delete-invoice"'),'Issued detail offers edit and delete');
const editHtml=await(await request(`/faktury/${saved.data.id}?edit=1`)).text();
check(editHtml.includes('Uložiť zmeny') && editHtml.includes('id="item-0-description"'),'Issued edit mode renders interactive document');
const change={...invoice.data,note:'HTTP edited invoice',items:[{...invoice.data.items[0],unitPrice:'50.00'}],qrProfiles:['epc']};
const edited=await post('invoice-save',change);
check(edited.ok && edited.data.number===invoice.data.number && edited.data.status==='issued' && edited.data.totals.gross===15375,'Issued edit saves same number and recalculated amount');
const editedBytes=Buffer.from(await(await request(`/faktury/${saved.data.id}/pdf`)).arrayBuffer());
check(createHash('sha256').update(editedBytes).digest('hex')!==createHash('sha256').update(bytes).digest('hex'),'Issued edit replaces stored PDF');
check((await post('invoice-save',invoice.data)).status===409,'Stale issued edit rejected over HTTP');
check((await post('invoice-delete',{id:saved.data.id,version:invoice.data.version})).status===409,'Stale issued delete rejected over HTTP');
const invalidEdit=await post('invoice-save',{...edited.data,items:[{...edited.data.items[0],description:''}]});
check(invalidEdit.status===422,'Incomplete issued edit rejected over HTTP');
const unchangedBytes=Buffer.from(await(await request(`/faktury/${saved.data.id}/pdf`)).arrayBuffer());
check(unchangedBytes.equals(editedBytes),'Failed issued edit preserves downloadable PDF');
await writeFile('temp/verification/http-edited-invoice.pdf',editedBytes);
const copy=await post('invoice-copy',{id:saved.data.id});check(copy.ok && copy.data.number===null,'Copy creates draft');
check((await post('invoice-delete',{id:copy.data.id,version:copy.data.version})).ok,'Owned test draft deletion');
check((await post('invoice-delete',{id:saved.data.id,version:edited.data.version})).ok,'Issued invoice deletion succeeds over HTTP');
check((await request(`/faktury/${saved.data.id}`)).status===404 && (await request(`/faktury/${saved.data.id}/pdf`)).status===404,'Deleted issued invoice and PDF return 404');
check((await post('invoice-save',edited.data)).status===404,'Deleted invoice cannot return through stale save');
check((await request('/faktury/9999999')).status===404,'Unknown invoice returns 404');
console.log(`\n${count} HTTP checks passed. Only isolated test data touched.`);
