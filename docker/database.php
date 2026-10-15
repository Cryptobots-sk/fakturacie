<?php
declare(strict_types=1);
// Run through Compose with --entrypoint php, so import can prepare an empty volume.
$target = dirname(__DIR__) . '/data/fakturacie.sqlite';
$command = $argv[1] ?? '';
function inspectDatabase(string $path): PDO {
    if (!is_file($path)) throw new RuntimeException('Databáza neexistuje.');
    $db = new PDO('sqlite:' . $path, options: [PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION]);
    if ($db->query('PRAGMA integrity_check')->fetchColumn() !== 'ok') throw new RuntimeException('Databáza neprešla kontrolou integrity.');
    foreach (['settings', 'customers', 'invoices', 'sequences'] as $table) $db->query('SELECT COUNT(*) FROM ' . $table)->fetchColumn();
    if ((int) $db->query('SELECT COUNT(*) FROM settings WHERE id=1')->fetchColumn() !== 1) throw new RuntimeException('Chýbajú nastavenia firmy.');
    return $db;
}
try {
    if ($command === 'import') {
        if (file_exists($target)) throw new RuntimeException('Úložisko už obsahuje databázu; import ju neprepíše.');
        $temporary = tempnam(dirname($target), '.import-');
        try {
            $out = fopen($temporary, 'wb');
            if ($out === false) throw new RuntimeException('Nedá sa otvoriť úložisko.');
            stream_copy_to_stream(STDIN, $out); fclose($out);
            $db = inspectDatabase($temporary);
            $db->exec('PRAGMA wal_checkpoint(TRUNCATE)');
            $db = null;
            chmod($temporary, 0600);
            if (!rename($temporary, $target)) throw new RuntimeException('Nedá sa uložiť databáza.');
            fwrite(STDOUT, "Databáza importovaná a overená.\n");
        } finally {
            foreach ([$temporary, $temporary . '-wal', $temporary . '-shm'] as $file) if (is_file($file)) unlink($file);
        }
    } elseif ($command === 'export') {
        $db = inspectDatabase($target);
        $temporary = sys_get_temp_dir() . '/fakturacie-' . bin2hex(random_bytes(8)) . '.sqlite';
        try {
            $db->exec('VACUUM INTO ' . $db->quote($temporary));
            readfile($temporary);
        } finally { if (is_file($temporary)) unlink($temporary); }
    } elseif ($command === 'manifest') {
        $db = inspectDatabase($target);
        $result = [];
        foreach (['settings', 'customers', 'invoices', 'sequences'] as $table) {
            $rows = $db->query('SELECT * FROM ' . $table . ' ORDER BY 1')->fetchAll(PDO::FETCH_ASSOC);
            $result[$table] = ['count' => count($rows), 'sha256' => hash('sha256', serialize($rows))];
        }
        echo json_encode($result, JSON_THROW_ON_ERROR | JSON_PRETTY_PRINT) . PHP_EOL;
    } else {
        throw new RuntimeException('Použitie: php docker/database.php import|export|manifest');
    }
} catch (Throwable $e) {
    fwrite(STDERR, $e->getMessage() . PHP_EOL);
    exit(1);
}
