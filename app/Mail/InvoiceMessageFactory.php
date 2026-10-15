<?php
declare(strict_types=1);
namespace App\Mail;
use App\Domain\{QrService, Rules};
use Latte\Engine;
use Nette\Mail\Message;

final class InvoiceMessageFactory
{
    public function __construct(private readonly QrService $qr) {}
    /** @param array<string, mixed> $smtp */
    public function envelope(array $smtp, string $to, string $subject): Message
    {
        $message = (new Message)->setFrom($smtp['fromEmail'], $smtp['fromName'])->addTo($to)->setSubject($subject);
        if ($smtp['replyTo'] !== '') $message->addReplyTo($smtp['replyTo']);
        return $message;
    }
    /** @param array<string, mixed> $invoice @param array<string, mixed> $smtp @param array<string, string> $content */
    public function create(array $invoice, string $pdf, array $smtp, array $content): Message
    {
        $message = $this->envelope($smtp, $content['to'], $content['subject']);
        $images = [];
        foreach ($this->qr->images($invoice) as $image) {
            $png = base64_decode(explode(',', $image['dataUri'], 2)[1], true);
            if ($png === false) throw new \RuntimeException('Invalid QR image');
            $part = $message->addEmbeddedFile('qr-' . $image['profile'] . '.png', $png, 'image/png');
            $images[] = ['src'=>'cid:' . trim((string)$part->getHeader('Content-ID'), '<>'), 'label'=>$image['label']];
        }
        $latte = new Engine;
        $latte->addFilter('money', Rules::money(...));
        $message->setHtmlBody($latte->renderToString(__DIR__ . '/invoice-email.latte', ['invoice'=>$invoice,'content'=>$content,'images'=>$images]));
        $message->setBody($content['message'] . "\n\nFaktúra " . $invoice['number'] . "\nDodávateľ: " . $invoice['supplier']['name']
            . "\nOdberateľ: " . $invoice['customer']['name'] . "\nSuma: " . Rules::money($invoice['totals']['gross'])
            . "\nSplatnosť: " . (new \DateTimeImmutable($invoice['dueDate']))->format('d.m.Y')
            . "\nIBAN: " . $invoice['supplier']['iban'] . "\nBIC / SWIFT: " . ($invoice['supplier']['bic'] ?: '—')
            . "\nVariabilný symbol: " . $invoice['number'] . "\nSpráva pre príjemcu: Faktura " . $invoice['number'] . "\nPDF faktúra je v prílohe.");
        $message->addAttachment('Faktura_' . $invoice['number'] . '.pdf', $pdf, 'application/pdf');
        return $message;
    }
}
