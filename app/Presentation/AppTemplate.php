<?php
declare(strict_types=1);
namespace App\Presentation;
use Nette\Bridges\ApplicationLatte\Template;
final class AppTemplate extends Template
{
    public string $title = 'Faktúry';
    public string $page = 'invoices';
    public string $csrf = '';
    public string $bootJson = '{}';
    public string $basePath = '';
    public string $baseUrl = '';
    public \Nette\Application\UI\Presenter $presenter;
    public \Nette\Application\UI\Control $control;
    /** @var array<mixed> */
    public array $flashes = [];
    /** @var list<array<string, mixed>> */
    public array $invoices = [];
    /** @var array<string, mixed> */
    public array $invoice = [];
    /** @var list<array<string, mixed>> */
    public array $customers = [];
    /** @var array<string, mixed> */
    public array $supplier = [];
    /** @var list<array<string, string>> */
    public array $qrImages = [];
    public bool $archived = false;
    public bool $editing = false;
    /** @var list<array<string, mixed>> */
    public array $deliveries = [];
}
