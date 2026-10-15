<?php
declare(strict_types=1);
date_default_timezone_set('Europe/Bratislava');
require dirname(__DIR__) . '/vendor/autoload.php';
$root = dirname(__DIR__);
$db = new App\Infrastructure\Database($root . '/data/fakturacie.sqlite');
$directory = $root . '/data/backups';
if (!is_dir($directory)) mkdir($directory, 0700, true);
$target = $directory . '/fakturacie-' . date('Ymd-His') . '-' . bin2hex(random_bytes(3)) . '.sqlite';
$db->pdo->exec('VACUUM INTO ' . $db->pdo->quote($target));
chmod($target, 0600);
echo 'Záloha: ' . $target . PHP_EOL;
