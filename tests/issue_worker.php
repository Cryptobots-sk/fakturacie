<?php
declare(strict_types=1);
require dirname(__DIR__) . '/vendor/autoload.php';
$db = new App\Infrastructure\Database($argv[1]);
$service = new App\Domain\InvoiceService($db, new App\Domain\CustomerService($db), new App\Domain\SettingsService($db), new App\Pdf\PdfService(new App\Domain\QrService, dirname(__DIR__) . '/temp'));
echo $service->issue((int) $argv[2], (int) $argv[3])['number'];
