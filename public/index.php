<?php
declare(strict_types=1);
// Compose publishes only 127.0.0.1. Docker NAT changes REMOTE_ADDR; the Host
// allowlist and the application CSRF/origin checks still apply in that runtime.
$host = strtolower((string) parse_url('http://' . ($_SERVER['HTTP_HOST'] ?? ''), PHP_URL_HOST));
$localPeer = in_array($_SERVER['REMOTE_ADDR'] ?? '', ['127.0.0.1', '::1'], true);
$dockerRuntime = getenv('FAKTURACIE_RUNTIME') === 'docker';
if (!in_array($host, ['127.0.0.1', 'localhost', '[::1]', 'faktura.local'], true) || (!$localPeer && !$dockerRuntime)) {
    http_response_code(403);
    exit('Aplikácia je dostupná iba na tomto počítači.');
}
header("Content-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'");
header('X-Content-Type-Options: nosniff');
header('Referrer-Policy: no-referrer');
header('Cache-Control: no-store');
require dirname(__DIR__) . '/vendor/autoload.php';
App\Bootstrap::boot()->getByType(Nette\Application\Application::class)->run();
