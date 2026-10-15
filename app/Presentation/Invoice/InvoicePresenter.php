<?php
declare(strict_types=1);
namespace App\Presentation\Invoice;
use App\Domain\{InvoiceService, CustomerService, QrService, InvoiceEmailService, Problem};
use App\Presentation\BasePresenter;
use Nette\Application\Responses\CallbackResponse;
use Nette\Http\{IRequest, IResponse};
final class InvoicePresenter extends BasePresenter
{
    public function __construct(private readonly InvoiceService $invoices, private readonly CustomerService $customers, private readonly QrService $qr, private readonly InvoiceEmailService $email) { parent::__construct(); }
    public function renderDefault(?int $id = null, bool $edit = false): void
    {
        try { $invoice = $id ? $this->invoices->get($id) : $this->invoices->blank(); }
        catch (Problem $e) { $this->error($e->getMessage(), $e->status); }
        $this->template->title = $invoice['number'] ? 'Faktúra ' . $invoice['number'] : 'Nová faktúra';
        $this->template->invoice = $invoice;
        $this->template->supplier = $invoice['supplier'];
        $this->template->customers = $this->customers->all();
        $this->template->editing = $invoice['status'] === 'draft' || $edit;
        $this->template->deliveries = $id ? $this->email->history($id) : [];
        $this->template->qrImages = $invoice['status'] === 'issued' ? $this->qr->images($invoice) : [];
        $this->bootData(['page' => 'editor', 'invoice' => $invoice, 'editing' => $this->template->editing, 'customers' => $this->template->customers, 'qrImages' => $this->template->qrImages, 'qrPlaceholder' => QrService::placeholder($invoice['status'] === 'issued' ? 'QR po uložení' : 'QR po vystavení')]);
    }
    public function actionPdf(int $id): void
    {
        try { $invoice = $this->invoices->get($id); $pdf = $this->invoices->pdf($id); }
        catch (Problem $e) { $this->error($e->getMessage(), $e->status); }
        $name = $invoice['number'] ? 'Faktura_' . $invoice['number'] : 'Koncept_' . $id;
        $download = $this->getParameter('download') === '1';
        $this->sendResponse(new CallbackResponse(static function (IRequest $request, IResponse $response) use ($pdf, $name, $download): void {
            $response->setContentType('application/pdf');
            $response->setHeader('Content-Disposition', ($download ? 'attachment' : 'inline') . '; filename="' . $name . '.pdf"');
            $response->setHeader('Content-Length', (string) strlen($pdf));
            echo $pdf;
        }));
    }
}
