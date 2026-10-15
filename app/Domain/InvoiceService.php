<?php
declare(strict_types=1);
namespace App\Domain;
use App\Infrastructure\Database;
use App\Pdf\PdfService;

final class InvoiceService
{
    public function __construct(private readonly Database $db, private readonly CustomerService $customers, private readonly SettingsService $settings, private readonly PdfService $pdf) {}

    /** @return array<string, mixed> */
    public function blank(): array
    {
        $supplier = $this->settings->get();
        $today = new \DateTimeImmutable('today');
        return ['id' => null, 'version' => 0, 'clientKey' => bin2hex(random_bytes(16)), 'number' => null, 'status' => 'draft', 'customerId' => null, 'customer' => null, 'supplier' => $supplier, 'issueDate' => $today->format('Y-m-d'), 'deliveryDate' => $today->format('Y-m-d'), 'dueDate' => $today->modify('+' . $supplier['paymentDays'] . ' days')->format('Y-m-d'), 'orderNumber' => '', 'currency' => 'EUR', 'note' => '', 'qrProfiles' => [$supplier['qrProfile']], 'items' => [['description' => '', 'quantity' => '1.000', 'unit' => 'ks', 'unitPrice' => '0.00', 'vatRate' => 23]], 'totals' => Rules::totals([])];
    }
    /** @return list<array<string, mixed>> */
    public function all(): array
    {
        return array_map($this->hydrate(...), $this->db->query('SELECT id, number, status, data, version, client_key, updated_at FROM invoices ORDER BY id DESC')->fetchAll());
    }
    /** @return array<string, mixed> */
    public function get(int $id): array
    {
        $row = $this->db->query('SELECT id, number, status, data, version, client_key, updated_at FROM invoices WHERE id = ?', [$id])->fetch();
        if (!$row) {
            throw new Problem('Faktúra sa nenašla.', 404);
        }
        return $this->hydrate($row);
    }
    /** @param array<string, mixed> $input @return array<string, mixed> */
    public function save(array $input): array
    {
        $key = $input['clientKey'] ?? '';
        if (!is_string($key) || !preg_match('/^[a-f0-9]{32}$/D', $key)) {
            throw new Problem('Neplatný identifikátor konceptu. Obnov stránku.');
        }
        if (!is_int($input['version'] ?? null)) {
            throw new Problem('Neplatná verzia faktúry.');
        }
        return $this->db->transaction(function () use ($input, $key): array {
            $row = $this->db->query('SELECT id FROM invoices WHERE client_key = ?', [$key])->fetch();
            $current = $row ? $this->get((int) $row['id']) : null;
            if (!$current && ($input['id'] ?? null) !== null) {
                throw new Problem('Faktúra sa nenašla.', 404);
            }
            if ($current && $current['id'] !== ($input['id'] ?? null)) {
                throw new Problem('Identifikátor faktúry nesúhlasí. Obnov stránku.', 409);
            }
            if (($current['version'] ?? 0) !== $input['version']) {
                throw new Problem('Faktúra sa zmenila v inom okne alebo sa predchádzajúce uloženie už dokončilo. Obnov stránku, aby si načítal uloženú verziu.', 409);
            }
            $issued = $current !== null && $current['status'] === 'issued';
            $data = Rules::invoice($input, $issued);
            $customer = $data['customerId'] ? $this->customers->get($data['customerId']) : null;
            if ($customer && $customer['archived'] && ($current['customerId'] ?? null) !== $customer['id']) {
                throw new Problem('Tento zákazník je archivovaný. Najprv ho obnov v adresári.');
            }
            // Preserve the chosen details unless the user explicitly refreshes or changes the customer.
            $data['customer'] = $current && $current['customerId'] === $data['customerId'] && empty($input['refreshCustomer']) ? $current['customer'] : $customer;
            $data['supplier'] = $current['supplier'] ?? $this->settings->get();
            if (!$current && ($input['supplier']['version'] ?? null) !== $data['supplier']['version']) {
                throw new Problem('Údaje tvojej firmy sa zmenili počas úpravy. Obnov stránku pred uložením konceptu.', 409);
            }
            $pdf = null;
            if ($issued) {
                $data['supplier'] = Rules::party($data['supplier'], true);
                $data['customer'] = Rules::party($data['customer']);
                // The existing number and status belong to the stored invoice.
                // Generate the replacement PDF before committing any edited data.
                $pdf = $this->pdf->render($data + ['number' => $current['number']]);
            }
            $now = date('c');
            if ($current) {
                $id = $current['id'];
                $statement = $this->db->pdo->prepare('UPDATE invoices SET data = :data, pdf = :pdf, version = version + 1, updated_at = :now WHERE id = :id');
                $statement->bindValue(':data', Database::json($data));
                $statement->bindValue(':pdf', $pdf, $pdf === null ? \PDO::PARAM_NULL : \PDO::PARAM_LOB);
                $statement->bindValue(':now', $now);
                $statement->bindValue(':id', $id, \PDO::PARAM_INT);
                $statement->execute();
            } else {
                $this->db->query("INSERT INTO invoices(client_key, status, data, created_at, updated_at) VALUES (?, 'draft', ?, ?, ?)", [$key, Database::json($data), $now, $now]);
                $id = (int) $this->db->pdo->lastInsertId();
            }
            return $this->get($id);
        });
    }
    /** @return array<string, mixed> */
    public function issue(int $id, int $version): array
    {
        return $this->db->transaction(function () use ($id, $version): array {
            $current = $this->get($id);
            // A retried issue request never assigns another number.
            if ($current['status'] === 'issued') {
                return $current;
            }
            if ($current['version'] !== $version) {
                throw new Problem('Koncept sa zmenil. Načítaj jeho aktuálnu verziu pred vystavením.', 409);
            }
            $data = Rules::invoice($current, true);
            $data['supplier'] = Rules::party($current['supplier'], true);
            $data['customer'] = Rules::party($current['customer']);
            $year = (int) substr($data['issueDate'], 0, 4);
            $this->db->query('INSERT OR IGNORE INTO sequences(year, last_number) VALUES (?, 0)', [$year]);
            $sequence = (int) $this->db->query('SELECT last_number FROM sequences WHERE year = ?', [$year])->fetchColumn() + 1;
            if ($sequence > 9999) {
                throw new Problem('Ročný číselný rad je vyčerpaný.');
            }
            $number = sprintf('%04d%04d', $year, $sequence);
            $pdf = $this->pdf->render($data + ['number' => $number]);
            $this->db->query('UPDATE sequences SET last_number = ? WHERE year = ?', [$sequence, $year]);
            $statement = $this->db->pdo->prepare("UPDATE invoices SET data = :data, number = :number, status = 'issued', version = version + 1, pdf = :pdf, updated_at = :now WHERE id = :id");
            $statement->bindValue(':data', Database::json($data));
            $statement->bindValue(':number', $number);
            $statement->bindValue(':pdf', $pdf, \PDO::PARAM_LOB);
            $statement->bindValue(':now', date('c'));
            $statement->bindValue(':id', $id, \PDO::PARAM_INT);
            $statement->execute();
            return $this->get($id);
        });
    }
    /** @return array<string, mixed> */
    public function copy(int $id): array
    {
        $original = $this->get($id);
        $blank = $this->blank();
        foreach (['customerId', 'items', 'note', 'qrProfiles'] as $key) {
            $blank[$key] = $original[$key];
        }
        if ($blank['customerId']) {
            $customer = $this->customers->get($blank['customerId']);
            if ($customer['archived']) {
                $blank['customerId'] = null;
            } else {
                $blank['dueDate'] = (new \DateTimeImmutable($blank['issueDate']))->modify('+' . $customer['paymentDays'] . ' days')->format('Y-m-d');
            }
        }
        return $this->save($blank);
    }
    public function delete(int $id, int $version): void
    {
        $this->db->transaction(function () use ($id, $version): void {
            $current = $this->get($id);
            if ($current['version'] !== $version) {
                throw new Problem('Faktúra sa zmenila v inom okne. Pred vymazaním obnov stránku.', 409);
            }
            $this->db->query('DELETE FROM invoices WHERE id = ? AND version = ?', [$id, $version]);
        });
    }
    public function pdf(int $id): string
    {
        $invoice = $this->get($id);
        if ($invoice['status'] === 'draft') {
            return $this->pdf->render($invoice, true);
        }
        $pdf = $this->db->query('SELECT pdf FROM invoices WHERE id = ?', [$id])->fetchColumn();
        if (!is_string($pdf) || $pdf === '') {
            throw new Problem('Uložené PDF sa nepodarilo načítať.', 500);
        }
        return $pdf;
    }
    /** @param array<string, mixed> $row @return array<string, mixed> */
    private function hydrate(array $row): array
    {
        return json_decode($row['data'], true, 512, JSON_THROW_ON_ERROR) + ['id' => (int) $row['id'], 'number' => $row['number'], 'status' => $row['status'], 'version' => (int) $row['version'], 'clientKey' => $row['client_key'], 'updatedAt' => $row['updated_at']];
    }
}
