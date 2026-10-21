import assert from 'node:assert/strict';
const base=process.env.TEST_URL||'http://127.0.0.1:8874';
const tlsUrl=process.env.MAILPIT_TLS_URL||'http://127.0.0.1:8027';
const sslUrl=process.env.MAILPIT_SSL_URL||'http://127.0.0.1:8028';
if(base!=='http://127.0.0.1:8874'||!['http://127.0.0.1:8027','http://127.0.0.1:8030'].includes(tlsUrl)||!['http://127.0.0.1:8028','http://127.0.0.1:8031'].includes(sslUrl))throw new Error('Only isolated SMTP security fixtures are permitted.');
const page=await fetch(base+'/nastavenia');const html=await page.text();assert.equal(page.status,200);
const token=html.match(/name="csrf-token" content="([^"]+)"/)[1];
const cookies=page.headers.getSetCookie().map(c=>c.split(';')[0]).join('; ');
const boot=JSON.parse(html.match(/id="app-data">([\s\S]*?)<\/script>/)[1]);
let version=boot.smtp.version;
async function post(action,data){const r=await fetch(base+'/api/'+action,{method:'POST',headers:{Cookie:cookies,'Content-Type':'application/json','X-CSRF-Token':token,Origin:base},body:JSON.stringify(data)});const result=await r.json();assert.equal(r.status,200);assert.ok(result.ok);return result.data;}
const count=async url=>(await(await fetch(url+'/api/v1/messages?limit=100')).json()).messages.length;
async function probe(host,encryption,password='fixture-password'){
 const config=await post('smtp-save',{host,port:1025,encryption,username:'fixture-mailer',password,fromEmail:'security@example.test',fromName:'SMTP security test',replyTo:'',version});version=config.version;
 return post('smtp-test',{to:'security-recipient@example.test',smtpVersion:version,requestKey:crypto.randomUUID().replaceAll('-','')});
}
if(process.argv[2]==='untrusted'){
 const before=await count(tlsUrl);const result=await probe('mailpit-tls','tls');assert.equal(result.status,'unknown');assert.equal(await count(tlsUrl),before);
 console.log('PASS untrusted TLS certificate rejected without delivery');
}else{
 let before=await count(tlsUrl);assert.equal((await probe('mailpit-tls','tls')).status,'sent');assert.equal(await count(tlsUrl),before+1);console.log('PASS verified STARTTLS and SMTP authentication');
 before=await count(tlsUrl);assert.equal((await probe('mailpit-tls','tls','wrong-fixture-password')).status,'unknown');assert.equal(await count(tlsUrl),before);console.log('PASS wrong SMTP password rejected without delivery');
 before=await count(sslUrl);assert.equal((await probe('mailpit-ssl','ssl')).status,'sent');assert.equal(await count(sslUrl),before+1);console.log('PASS verified implicit SSL/TLS and SMTP authentication');
}
