<?php
declare(strict_types=1);
$path = rawurldecode(parse_url($_SERVER['REQUEST_URI'] ?? '/', PHP_URL_PATH) ?: '/');
$file = realpath(__DIR__ . $path);
if ($file && str_starts_with($file, __DIR__ . '/assets/') && is_file($file)) {
    return false;
}
require __DIR__ . '/index.php';
