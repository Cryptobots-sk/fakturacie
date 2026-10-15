<?php
declare(strict_types=1);
require dirname(__DIR__) . '/vendor/autoload.php';
umask(0077);
$root = dirname(__DIR__);
foreach (['data', 'log', 'temp', 'temp/sessions', 'temp/mpdf', 'temp/latte-pdf'] as $dir) { if (!is_dir($root . '/' . $dir)) { mkdir($root . '/' . $dir, 0700, true); } }
$path = getenv('FAKTURACIE_DB') ?: $root . '/data/fakturacie.sqlite';
if (!is_dir(dirname($path))) { mkdir(dirname($path), 0700, true); }
$db = new App\Infrastructure\Database($path);
$db->migrate();
$db->transaction(function () use ($db): void {
    if ($db->query('SELECT COUNT(*) FROM settings')->fetchColumn() > 0) { return; }
    // The first run contains no company, bank account, customer or invoice data.
    // Saving company settings still requires the full Rules::party validation.
    $supplier = array_fill_keys(['name', 'street', 'zip', 'city', 'ico', 'dic', 'vatId', 'email', 'iban', 'bic', 'registry'], '');
    $supplier += ['country'=>'Slovensko', 'paymentDays'=>14, 'qrProfile'=>'sk'];
    $db->query('INSERT INTO settings(id, data) VALUES (1, ?)', [App\Infrastructure\Database::json($supplier)]);
});
echo "Pripravené. Pri prvom spustení vyplň údaje v časti Moja firma.\n";
