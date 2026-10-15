<?php
declare(strict_types=1);
namespace App\Presentation\Home;
use App\Domain\InvoiceService;
use App\Presentation\BasePresenter;
final class HomePresenter extends BasePresenter
{
    public function __construct(private readonly InvoiceService $invoices) { parent::__construct(); }
    public function renderDefault(): void
    {
        $this->template->invoices = $this->invoices->all();
        $this->bootData(['page' => 'invoices']);
    }
}
