<?php
declare(strict_types=1);
namespace App\Pdf;
use App\Domain\QrService;
use App\Domain\Rules;
use Latte\Engine;
use Mpdf\Mpdf;

final class PdfService
{
    public function __construct(private readonly QrService $qr, private readonly string $tempDir) {}
    /** @param array<string, mixed> $invoice */
    public function render(array $invoice, bool $draft = false): string
    {
        $latte = new Engine;
        $latte->setTempDirectory($this->tempDir . '/latte-pdf');
        $latte->addFilter('money', Rules::money(...));
        $latte->addFilter('invoiceDate', static fn(string $date): string => $date === '' ? '—' : (new \DateTimeImmutable($date))->format('d.m.Y'));
        $mpdf = new Mpdf(['tempDir' => $this->tempDir . '/mpdf', 'format' => 'A4', 'margin_left' => 17, 'margin_right' => 17, 'margin_top' => 18, 'margin_bottom' => 18, 'default_font' => 'dejavusans', 'default_font_size' => 9]);
        $mpdf->showImageErrors = true;
        $mpdf->SetTitle($draft ? 'Koncept faktúry' : 'Faktúra ' . $invoice['number']);
        $mpdf->SetAuthor($invoice['supplier']['name']);
        $mpdf->SetHTMLFooter('<div class="document-footer">' . htmlspecialchars($invoice['supplier']['name'], ENT_QUOTES, 'UTF-8') . ' · ' . ($draft ? 'KONCEPT' : 'Faktúra ' . $invoice['number']) . '<span> | {PAGENO}/{nbpg}</span></div>');
        $html = $latte->renderToString(__DIR__ . '/invoice.latte', ['invoice' => $invoice, 'draft' => $draft, 'qrImages' => $draft ? [] : $this->qr->images($invoice), 'documentCss' => file_get_contents(dirname(__DIR__, 2) . '/assets/invoice.css')]);
        $mpdf->WriteHTML($html);
        return $mpdf->OutputBinaryData();
    }
}
