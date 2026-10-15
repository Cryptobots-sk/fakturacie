<?php
declare(strict_types=1);
namespace App\Domain;
use App\Infrastructure\Database;
final class SettingsService
{
    public function __construct(private readonly Database $db) {}
    /** @return array<string, mixed> */
    public function get(): array
    {
        $row = $this->db->query('SELECT * FROM settings WHERE id = 1')->fetch();
        if (!$row) {
            throw new Problem('Najprv spusti prípravu aplikácie.', 503);
        }
        return json_decode($row['data'], true, 512, JSON_THROW_ON_ERROR) + ['version' => (int) $row['version']];
    }
    /** @param array<string, mixed> $input @return array<string, mixed> */
    public function save(array $input): array
    {
        $data = Rules::party($input, true);
        if (!is_int($input['version'] ?? null)) {
            throw new Problem('Neplatná verzia nastavení.');
        }
        if ($this->db->query('UPDATE settings SET data = ?, version = version + 1 WHERE id = 1 AND version = ?', [Database::json($data), $input['version']])->rowCount() !== 1) {
            throw new Problem('Nastavenia sa zmenili v inom okne. Obnov stránku.', 409);
        }
        return $this->get();
    }
}
