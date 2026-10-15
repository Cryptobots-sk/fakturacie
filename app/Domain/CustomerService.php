<?php
declare(strict_types=1);
namespace App\Domain;
use App\Infrastructure\Database;

final class CustomerService
{
    public function __construct(private readonly Database $db) {}
    /** @return list<array<string, mixed>> */
    public function all(bool $archived = false): array
    {
        $rows = $this->db->query('SELECT * FROM customers WHERE archived = ? ORDER BY id DESC', [(int) $archived])->fetchAll();
        $customers = array_map($this->hydrate(...), $rows);
        usort($customers, static fn(array $a, array $b): int => strnatcasecmp($a['name'], $b['name']));
        return $customers;
    }
    /** @return array<string, mixed> */
    public function get(int $id): array
    {
        $row = $this->db->query('SELECT * FROM customers WHERE id = ?', [$id])->fetch();
        if (!$row) {
            throw new Problem('Zákazník sa nenašiel.', 404);
        }
        return $this->hydrate($row);
    }
    /** @param array<string, mixed> $input @return array<string, mixed> */
    public function save(array $input): array
    {
        $data = Rules::party($input);
        return $this->db->transaction(function () use ($input, $data): array {
            $id = $input['id'] ?? null;
            if ($id !== null) {
                if (!is_int($id) || $id < 1) {
                    throw new Problem('Neplatné číslo zákazníka.');
                }
                $current = $this->get($id);
                if ($current['version'] !== ($input['version'] ?? null)) {
                    throw new Problem('Zákazník sa zmenil v inom okne. Obnov stránku a skús znovu.', 409);
                }
                $this->db->query('UPDATE customers SET data = ?, version = version + 1 WHERE id = ?', [Database::json($data), $id]);
            } else {
                $this->db->query('INSERT INTO customers(data) VALUES (?)', [Database::json($data)]);
                $id = (int) $this->db->pdo->lastInsertId();
            }
            return $this->get($id);
        });
    }
    public function archive(int $id, int $version, bool $archived): void
    {
        $affected = $this->db->query('UPDATE customers SET archived = ?, version = version + 1 WHERE id = ? AND version = ?', [(int) $archived, $id, $version])->rowCount();
        if ($affected !== 1) {
            throw new Problem('Zákazník sa zmenil. Obnov stránku a skús znovu.', 409);
        }
    }
    /** @param array<string, mixed> $row @return array<string, mixed> */
    private function hydrate(array $row): array
    {
        return json_decode($row['data'], true, 512, JSON_THROW_ON_ERROR) + ['id' => (int) $row['id'], 'version' => (int) $row['version'], 'archived' => (bool) $row['archived']];
    }
}
