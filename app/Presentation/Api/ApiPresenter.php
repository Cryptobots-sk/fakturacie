<?php
declare(strict_types=1);
namespace App\Presentation\Api;
use App\Domain\{CustomerService, InvoiceService, SettingsService, SmtpSettingsService, InvoiceEmailService, Problem};
use App\Infrastructure\RequestGuard;
use Nette\Application\UI\Presenter;
final class ApiPresenter extends Presenter
{
    public function __construct(private readonly RequestGuard $guard, private readonly CustomerService $customers, private readonly InvoiceService $invoices, private readonly SettingsService $settings, private readonly SmtpSettingsService $smtp, private readonly InvoiceEmailService $email) { parent::__construct(); }
    public function actionDefault(): void { $this->error('Neznáma operácia.', 404); }
    protected function startup(): void
    {
        parent::startup();
        try {
            $body = $this->guard->body();
            $result = match ($this->getAction()) {
                'customerSave' => $this->customers->save($body),
                'customerArchive' => $this->archive($body),
                'settingsSave' => $this->settings->save($body),
                'smtpSave' => $this->smtp->save($body),
                'smtpTest' => $this->email->test($body),
                'invoiceEmailPreview' => $this->email->preview($this->integer($body,'id'),$this->integer($body,'version')),
                'invoiceEmailSend' => $this->email->send($body),
                'invoiceSave' => $this->invoices->save($body),
                'invoiceIssue' => $this->invoices->issue($this->integer($body, 'id'), $this->integer($body, 'version')),
                'invoiceCopy' => $this->invoices->copy($this->integer($body, 'id')),
                'invoiceDelete' => $this->delete($body),
                default => throw new Problem('Neznáma operácia.', 404),
            };
        } catch (Problem $e) {
            $this->getHttpResponse()->setCode($e->status);
            $this->sendJson(['ok' => false, 'error' => $e->getMessage(), 'fields' => $e->fields]);
        } catch (\Throwable $e) {
            if (in_array($this->getAction(), ['smtpSave','smtpTest','invoiceEmailSend'], true)) {
                \Tracy\Debugger::log('E-mail operation failed: ' . $e::class);
            } else {
                \Tracy\Debugger::log($e);
            }
            $this->getHttpResponse()->setCode(500);
            $this->sendJson(['ok' => false, 'error' => 'Operáciu sa nepodarilo dokončiť. Údaje v editore zostali zachované; skús to znova.']);
        }
        $this->sendJson(['ok' => true, 'data' => $result]);
    }
    /** @param array<string, mixed> $body */
    private function integer(array $body, string $key): int
    {
        if (!is_int($body[$key] ?? null) || $body[$key] < 1) {
            throw new Problem('Chýba platný identifikátor alebo verzia.');
        }
        return $body[$key];
    }
    /** @param array<string, mixed> $body @return array<string, bool> */
    private function archive(array $body): array
    {
        if (!is_bool($body['archived'] ?? null)) { throw new Problem('Neplatný stav zákazníka.'); }
        $this->customers->archive($this->integer($body, 'id'), $this->integer($body, 'version'), $body['archived']);
        return ['saved' => true];
    }
    /** @param array<string, mixed> $body @return array<string, bool> */
    private function delete(array $body): array
    {
        $this->invoices->delete($this->integer($body, 'id'), $this->integer($body, 'version'));
        return ['deleted' => true];
    }
}
