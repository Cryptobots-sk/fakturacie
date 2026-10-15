<?php
declare(strict_types=1);
require dirname(__DIR__) . '/vendor/autoload.php';
$path = getenv('FAKTURACIE_DB') ?: dirname(__DIR__) . '/data/fakturacie.sqlite';
if (!is_file($path)) throw new RuntimeException('Chýba databáza; najprv ju obnov zo zálohy.');
(new App\Infrastructure\Database($path))->migrate();
