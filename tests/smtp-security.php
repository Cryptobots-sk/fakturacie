<?php
declare(strict_types=1);
require dirname(__DIR__) . '/vendor/autoload.php';
use App\Mail\SmtpTransport;
use Nette\Mail\{Message,SmtpException};
$transport=new SmtpTransport;
$settings=['host'=>'mailpit-tls','port'=>1025,'encryption'=>'tls','username'=>'fixture-mailer','password'=>'fixture-password'];
$message=static fn(string $subject)=>(new Message)->setFrom('security@example.test')->addTo('security-recipient@example.test')->setSubject($subject)->setBody('Local SMTP security test.');
if(($argv[1] ?? '')==='untrusted') {
 try {$transport->send($message('Untrusted certificate must fail'),$settings);throw new RuntimeException('Untrusted certificate accepted');}
 catch(SmtpException) {echo "PASS untrusted TLS certificate rejected\n";}
 exit;
}
$transport->send($message('Verified STARTTLS and AUTH'),$settings);
echo "PASS verified STARTTLS and SMTP authentication\n";
try {$transport->send($message('Wrong password must fail'),array_replace($settings,['password'=>'wrong-fixture-password']));throw new RuntimeException('Wrong SMTP password accepted');}
catch(SmtpException) {echo "PASS wrong SMTP password rejected\n";}
$transport->send($message('Verified implicit TLS and AUTH'),array_replace($settings,['host'=>'mailpit-ssl','encryption'=>'ssl']));
echo "PASS verified implicit SSL/TLS and SMTP authentication\n";
