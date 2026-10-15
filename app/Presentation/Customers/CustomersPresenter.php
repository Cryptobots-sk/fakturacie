<?php
declare(strict_types=1);
namespace App\Presentation\Customers;
use App\Domain\CustomerService;
use App\Presentation\BasePresenter;
final class CustomersPresenter extends BasePresenter
{
    public function __construct(private readonly CustomerService $customers) { parent::__construct(); }
    public function renderDefault(bool $archived = false): void
    {
        $this->template->title = 'Zákazníci';
        $this->template->page = 'customers';
        $this->template->archived = $archived;
        $this->template->customers = $this->customers->all($archived);
        $this->bootData(['page' => 'customers', 'customers' => $this->template->customers, 'archived' => $archived]);
    }
}
