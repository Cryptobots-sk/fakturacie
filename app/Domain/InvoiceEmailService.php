<?php
declare(strict_types=1);
namespace App\Domain;
use App\Infrastructure\Database;
use App\Mail\{InvoiceMessageFactory, SmtpTransport};
use Nette\Mail\Message;

final class InvoiceEmailService
{
    public function __construct(private readonly Database $db, private readonly InvoiceService $invoices,
        private readonly SmtpSettingsService $smtp, private readonly QrService $qr,
        private readonly InvoiceMessageFactory $messages, private readonly SmtpTransport $transport) {}
    /** @return array<string, mixed> */
    public function preview(int $id, int $version): array
    {
        $invoice = $this->invoices->get($id);
        $this->checkInvoice($invoice, $version);
        $smtp = $this->smtp->get();
        return ['requestKey'=>bin2hex(random_bytes(16)), 'invoiceVersion'=>$version, 'smtpVersion'=>$smtp['version'],
            'configured'=>$smtp['configured'], 'fromEmail'=>$smtp['fromEmail'],
            'to'=>$invoice['customer']['email'] ?? '', 'subject'=>'Faktúra ' . $invoice['number'] . ' – ' . $invoice['supplier']['name'],
            'message'=>"Dobrý deň,\n\nv prílohe Vám posielame faktúru č. " . $invoice['number'] . ". Údaje k platbe nájdete nižšie.\n\nĎakujeme za spoluprácu.\n" . $invoice['supplier']['name'],
            'number'=>$invoice['number'], 'customer'=>$invoice['customer']['name'], 'amount'=>Rules::money($invoice['totals']['gross']),
            'dueDate'=>(new \DateTimeImmutable($invoice['dueDate']))->format('d.m.Y'), 'iban'=>implode(' ',str_split($invoice['supplier']['iban'],4)),
            'bic'=>$invoice['supplier']['bic'], 'qrImages'=>$this->qr->images($invoice), 'attachment'=>'Faktura_' . $invoice['number'] . '.pdf'];
    }
    /** @return list<array<string, mixed>> */
    public function history(int $id): array
    {
        return $this->db->query('SELECT id, recipient, subject, status, created_at, finished_at, invoice_version FROM email_deliveries WHERE invoice_id=? ORDER BY id DESC LIMIT 10', [$id])->fetchAll();
    }
    /** @param array<string, mixed> $input @return array<string, mixed> */
    public function send(array $input): array
    {
        $id = $input['id'] ?? null; $version = $input['version'] ?? null;
        if (!is_int($id) || $id < 1 || !is_int($version) || $version < 1) throw new Problem('Chýba platná faktúra alebo verzia.');
        $content = ['to'=>SmtpSettingsService::email($input['to'] ?? '', 'Príjemca'),
            'subject'=>Rules::text($input['subject'] ?? '', 180, 'Predmet', true),
            'message'=>Rules::text($input['message'] ?? '', 4000, 'Správa', true, true)];
        return $this->deliver($input, $id, $version, $content);
    }
    /** @param array<string, mixed> $input @return array<string, mixed> */
    public function test(array $input): array
    {
        $content = ['to'=>SmtpSettingsService::email($input['to'] ?? '', 'Príjemca skúšobného e-mailu'),
            'subject'=>'Skúška SMTP – Fakturácie', 'message'=>'Toto je skúšobný e-mail z aplikácie Fakturácie. SMTP server prijal túto správu.'];
        return $this->deliver($input, null, null, $content);
    }
    /** @param array<string, mixed> $input @param array<string, string> $content @return array<string, mixed> */
    private function deliver(array $input, ?int $invoiceId, ?int $version, array $content): array
    {
        $key = $input['requestKey'] ?? '';
        if (!is_string($key) || !preg_match('/^[a-f0-9]{32}$/D', $key)) throw new Problem('Neplatná požiadavka na odoslanie. Otvor e-mail znova.');
        $hash = hash('sha256', Database::json([$invoiceId,$version,$input['smtpVersion'] ?? null,$content]));
        $prepared = $this->db->transaction(function () use ($input,$invoiceId,$version,$content,$key,$hash): array {
            $existing = $this->db->query('SELECT * FROM email_deliveries WHERE request_key=?',[$key])->fetch();
            if ($existing) {
                if ($existing['payload_hash'] !== $hash) throw new Problem('Táto požiadavka už bola použitá pre inú správu. Otvor nový e-mail.',409);
                return ['existing'=>$existing];
            }
            $smtp = $this->smtp->connection();
            if (($input['smtpVersion'] ?? null) !== $smtp['version']) throw new Problem('SMTP nastavenia sa zmenili. Obnov náhľad správy.',409);
            if ($smtp['host'] === '' || $smtp['fromEmail'] === '') throw new Problem('Najprv nastav SMTP server v časti Moja firma.');
            if ($invoiceId !== null) {
                $invoice = $this->invoices->get($invoiceId);
                $this->checkInvoice($invoice, $version);
                $pdf = $this->invoices->pdf($invoiceId);
                $message = $this->messages->create($invoice,$pdf,$smtp,$content);
                $pdfHash = hash('sha256',$pdf);
            } else {
                $message = $this->messages->envelope($smtp,$content['to'],$content['subject'])->setBody($content['message']);
                $pdfHash = null;
            }
            $this->db->query("INSERT INTO email_deliveries(request_key,payload_hash,invoice_id,invoice_version,recipient,subject,status,pdf_sha256,created_at) VALUES(?,?,?,?,?,?,'sending',?,?)",
                [$key,$hash,$invoiceId,$version,$content['to'],$content['subject'],$pdfHash,date('c')]);
            return ['id'=>(int)$this->db->pdo->lastInsertId(),'message'=>$message,'smtp'=>$smtp];
        });
        if (isset($prepared['existing'])) return $this->result($prepared['existing']);
        // Never hold a database transaction while waiting on an external SMTP server.
        try {
            $this->transport->send($prepared['message'],$prepared['smtp']);
            $status = 'sent';
        } catch (\Throwable) {
            // A lost SMTP acknowledgement can occur after delivery. Do not resend automatically.
            // Do not log SMTP exceptions or credentials into application/Tracy logs.
            $status = 'unknown';
        }
        $this->db->query('UPDATE email_deliveries SET status=?,finished_at=? WHERE id=?',[$status,date('c'),$prepared['id']]);
        return $this->result($this->db->query('SELECT * FROM email_deliveries WHERE id=?',[$prepared['id']])->fetch());
    }
    /** @param array<string, mixed> $invoice */
    private function checkInvoice(array $invoice, ?int $version): void
    {
        if ($invoice['version'] !== $version) throw new Problem('Faktúra sa zmenila. Obnov stránku a priprav e-mail znova.',409);
        if ($invoice['status'] !== 'issued') throw new Problem('Pre odoslanie najprv vystav faktúru, aby mala číslo a platobné údaje.');
    }
    /** @param array<string, mixed> $row @return array<string, mixed> */
    private function result(array $row): array
    {
        return ['id'=>(int)$row['id'],'status'=>$row['status'],'recipient'=>$row['recipient'],
            'message'=>match($row['status']) {
                'sent'=>'SMTP server správu prijal.',
                'sending'=>'Požiadavka už bola prijatá. Odoslanie sa ešte nepotvrdilo; skontroluj históriu pred ďalším pokusom.',
                default=>'Odoslanie sa nepotvrdilo. Skontroluj SMTP nastavenia a schránku príjemcu pred novým pokusom.',
            }];
    }
}
