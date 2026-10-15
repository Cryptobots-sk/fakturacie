<?php
declare(strict_types=1);
require dirname(__DIR__) . '/vendor/autoload.php';
use App\Domain\{SmtpSettingsService, InvoiceService, SettingsService, CustomerService, QrService, InvoiceEmailService, Problem};
use App\Infrastructure\Database;
use App\Mail\{InvoiceMessageFactory,SmtpTransport};
use App\Pdf\PdfService;
$root=dirname(__DIR__);$folder=$root.'/temp/verification';if(!is_dir($folder))mkdir($folder,0700,true);
$db=new Database($folder.'/email-'.bin2hex(random_bytes(5)).'.sqlite');$db->migrate();
$smtp=new SmtpSettingsService($db);$checks=0;
function check(bool $yes,string $label):void {global $checks;if(!$yes)throw new RuntimeException($label);$checks++;echo "PASS $label\n";}
function reject(callable $fn,int $code,string $label):void {try{$fn();}catch(Problem $e){check($e->status===$code,$label);return;}throw new RuntimeException('Missing rejection: '.$label);}
check($smtp->get()['configured']===false && !array_key_exists('password',$smtp->get()),'Unconfigured SMTP never exposes password');
$config=['host'=>'smtp.example.test','port'=>587,'encryption'=>'tls','username'=>'account','password'=>'secret-fixture','fromEmail'=>'sender@example.test','fromName'=>'Test <odosielateľ>','replyTo'=>'reply@example.test','version'=>0];
$saved=$smtp->save($config);check($saved['passwordSet'] && !array_key_exists('password',$saved),'Saved password redacted from public settings');
$saved=$smtp->save(array_replace($config,['version'=>$saved['version'],'password'=>'']));check($smtp->connection()['password']==='secret-fixture','Blank password preserves existing credential');
reject(fn()=>$smtp->save($config),409,'Stale SMTP settings rejected');
reject(fn()=>$smtp->save(array_replace($config,['version'=>$saved['version'],'fromEmail'=>"sender@example.test\r\nBcc:bad@example.test"])),422,'SMTP address/header injection rejected');
reject(fn()=>$smtp->save(array_replace($config,['version'=>$saved['version'],'host'=>'https://bad.test/path'])),422,'SMTP URL rejected');
reject(fn()=>$smtp->save(array_replace($config,['version'=>$saved['version'],'port'=>70000])),422,'Invalid SMTP port rejected');
reject(fn()=>$smtp->save(array_replace($config,['version'=>$saved['version'],'host'=>'new.example.test','password'=>''])),422,'Host change never reuses prior password');
$qr=new QrService;$factory=new InvoiceMessageFactory($qr);$invoice=['number'=>'20260099','supplier'=>['name'=>'Test dodávateľ','iban'=>'SK5000000000000000000001','bic'=>'TESTSK00'],'customer'=>['name'=>'Odberateľ <script>'],'dueDate'=>'2026-10-29','totals'=>['gross'=>354240],'qrProfiles'=>['sk','epc']];
$message=$factory->create($invoice,'%PDF-fixture',$smtp->connection(),['to'=>'customer@example.test','subject'=>'Faktúra 20260099','message'=>"Dobrý deň,\n<script>alert(1)</script>"]);
$raw=$message->generateMessage();file_put_contents($folder.'/email-unit.eml',$raw);
check(str_contains($raw,'multipart/mixed') && str_contains($raw,'multipart/related') && str_contains($raw,'multipart/alternative'),'Email has PDF, inline images and HTML/text alternatives');
check(str_contains($raw,'Content-ID:') && str_contains($raw,'application/pdf') && str_contains($raw,'image/png'),'Embedded PNG QR and PDF MIME types');
check(!str_contains($message->getHtmlBody(),'<script>') && str_contains($message->getHtmlBody(),'&lt;script&gt;'),'Email HTML escapes editable content');
$smtp->save(array_replace($saved,['username'=>'','password'=>'','clearPassword'=>true]));check(!$smtp->get()['passwordSet'],'Explicit password removal');
echo "$checks e-mail domain checks passed.\n";
