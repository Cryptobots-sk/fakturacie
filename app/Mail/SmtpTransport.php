<?php
declare(strict_types=1);
namespace App\Mail;
use Nette\Mail\{Message, SmtpMailer};
final class SmtpTransport
{
    /** @param array<string, mixed> $settings */
    public function send(Message $message, #[\SensitiveParameter] array $settings): void
    {
        $mailer = new SmtpMailer(host:$settings['host'], username:$settings['username'], password:$settings['password'],
            port:$settings['port'], encryption:$settings['encryption'] === 'none' ? null : $settings['encryption'], timeout:15,
            clientHost:'fakturacie.local', streamOptions:['ssl'=>['verify_peer'=>true,'verify_peer_name'=>true]]);
        $mailer->send($message);
    }
}
