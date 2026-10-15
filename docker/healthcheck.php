<?php
declare(strict_types=1);
try {
    $file = dirname(__DIR__) . '/data/fakturacie.sqlite';
    if (!is_file($file)) throw new RuntimeException('Missing database');
    $db = new PDO('sqlite:' . $file, options: [PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION]);
    if ((int) $db->query('SELECT COUNT(*) FROM settings WHERE id=1')->fetchColumn() !== 1) throw new RuntimeException('Missing settings');
    // Reuse one healthcheck session instead of creating thousands of session files.
    $cookieFile = dirname(__DIR__) . '/temp/.health-session';
    $session = is_file($cookieFile) ? trim((string) file_get_contents($cookieFile)) : '';
    $headers = preg_match('/^[a-zA-Z0-9,-]+$/D', $session) ? 'Cookie: _nss=1; PHPSESSID=' . $session : '';
    $context = stream_context_create(['http' => ['timeout' => 5, 'header' => $headers]]);
    $body = @file_get_contents('http://127.0.0.1/', false, $context);
    if ($body === false || !str_contains($body, 'id="app-data"')) throw new RuntimeException('Application HTTP unavailable');
    foreach (http_get_last_response_headers() ?? [] as $header) {
        if (preg_match('/^Set-Cookie: PHPSESSID=([a-zA-Z0-9,-]+);/i', $header, $matches)) {
            file_put_contents($cookieFile, $matches[1]);
            chmod($cookieFile, 0600);
        }
    }
    exit(0);
} catch (Throwable $e) {
    fwrite(STDERR, $e->getMessage() . PHP_EOL);
    exit(1);
}
