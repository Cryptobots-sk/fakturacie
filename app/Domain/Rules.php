<?php
declare(strict_types=1);
namespace App\Domain;

final class Rules
{
    public const QR_LABELS = ['sk' => 'PAY by square · Slovensko', 'cz' => 'QR Platba · Česko', 'epc' => 'SEPA QR · Európa'];

    public static function text(mixed $value, int $max, string $label, bool $required = false, bool $multiline = false): string
    {
        if (!is_string($value)) {
            throw new Problem("Pole „{$label}“ musí obsahovať text.");
        }
        $value = trim($value);
        $pattern = $multiline ? '/[\x00-\x08\x0b-\x1f\x7f]/u' : '/[\x00-\x1f\x7f]/u';
        if (!mb_check_encoding($value, 'UTF-8') || mb_strlen($value) > $max || preg_match($pattern, $value)) {
            throw new Problem("Skontroluj pole „{$label}“ (najviac $max znakov, bez riadiacich znakov).");
        }
        if ($required && $value === '') {
            throw new Problem("Vyplň pole „{$label}“.");
        }
        return $value;
    }

    /** @param array<string, mixed> $input @return array<string, mixed> */
    public static function party(array $input, bool $supplier = false): array
    {
        $result = [];
        foreach (['name' => [70, 'Názov firmy'], 'street' => [120, 'Ulica a číslo'], 'zip' => [20, 'PSČ'], 'city' => [80, 'Mesto'], 'country' => [70, 'Krajina'], 'ico' => [30, 'IČO'], 'dic' => [30, 'DIČ'], 'vatId' => [30, 'IČ DPH'], 'email' => [160, 'E-mail']] as $field => [$max, $label]) {
            $result[$field] = self::text($input[$field] ?? '', $max, $label, in_array($field, ['name', 'street', 'zip', 'city', 'country'], true));
        }
        if ($result['email'] !== '' && !filter_var($result['email'], FILTER_VALIDATE_EMAIL)) {
            throw new Problem('Zadaj platnú e-mailovú adresu.');
        }
        $days = filter_var($input['paymentDays'] ?? 14, FILTER_VALIDATE_INT);
        if ($days === false || $days < 0 || $days > 365) {
            throw new Problem('Splatnosť musí byť od 0 do 365 dní.');
        }
        $result['paymentDays'] = $days;
        $profile = $input['qrProfile'] ?? 'sk';
        if (!is_string($profile) || !isset(self::QR_LABELS[$profile])) {
            throw new Problem('Vyber platný typ QR kódu.');
        }
        $result['qrProfile'] = $profile;
        if ($supplier) {
            $iban = strtoupper(str_replace(' ', '', self::text($input['iban'] ?? '', 42, 'IBAN', true)));
            if (!self::validIban($iban)) {
                throw new Problem('IBAN nie je platný. Skontroluj číslo účtu.');
            }
            $bic = strtoupper(self::text($input['bic'] ?? '', 11, 'BIC'));
            if ($bic !== '' && !preg_match('/^[A-Z]{6}[A-Z0-9]{2}([A-Z0-9]{3})?$/D', $bic)) {
                throw new Problem('BIC musí mať 8 alebo 11 znakov.');
            }
            $result += ['iban' => $iban, 'bic' => $bic, 'registry' => self::text($input['registry'] ?? '', 300, 'Zápis v registri')];
        }
        return $result;
    }

    public static function validIban(string $iban): bool
    {
        if (!preg_match('/^[A-Z]{2}[0-9]{2}[A-Z0-9]{11,30}$/D', $iban)) {
            return false;
        }
        $lengths = ['SK'=>24,'CZ'=>24,'DE'=>22,'AT'=>20,'BE'=>16,'FR'=>27,'NL'=>18,'IE'=>22,'IT'=>27,'ES'=>24,'PT'=>25,'PL'=>28,'HU'=>28,'RO'=>24,'BG'=>22,'HR'=>21,'SI'=>19,'EE'=>20,'LV'=>21,'LT'=>20,'FI'=>18,'SE'=>24,'DK'=>18,'LU'=>20,'MT'=>31,'CY'=>28,'GR'=>27,'GB'=>22,'CH'=>21,'NO'=>15,'IS'=>26,'LI'=>21];
        if (isset($lengths[substr($iban, 0, 2)]) && strlen($iban) !== $lengths[substr($iban, 0, 2)]) {
            return false;
        }
        $digits = preg_replace_callback('/[A-Z]/', static fn(array $m): string => (string) (ord($m[0]) - 55), substr($iban, 4) . substr($iban, 0, 4));
        return bcmod((string) $digits, '97') === '1';
    }

    public static function date(mixed $value, string $label, bool $required = false): string
    {
        if (!$required && $value === '') {
            return '';
        }
        if (!is_string($value)) {
            throw new Problem("Skontroluj dátum „{$label}“.");
        }
        $date = \DateTimeImmutable::createFromFormat('!Y-m-d', $value);
        if (!$date || $date->format('Y-m-d') !== $value || $value < '2000-01-01' || $value > '2099-12-31') {
            throw new Problem("Skontroluj dátum „{$label}“.");
        }
        return $value;
    }

    public static function decimal(mixed $value, int $scale, string $label, string $max): string
    {
        if (!is_string($value) && !is_int($value)) {
            throw new Problem("Pole „{$label}“ musí obsahovať číslo.");
        }
        $value = str_replace([' ', ',', "\u{00a0}"], ['', '.', ''], trim((string) $value));
        if ($value === '') {
            $value = '0';
        }
        if (!preg_match('/^\d{1,9}(\.\d{1,' . $scale . '})?$/D', $value) || bccomp($value, $max, $scale) > 0) {
            throw new Problem("Skontroluj pole „{$label}“ (kladné číslo, najviac $scale desatinné miesta).");
        }
        return bcadd($value, '0', $scale);
    }

    /** @param array<string, mixed> $input @return array<string, mixed> */
    public static function invoice(array $input, bool $issuing = false): array
    {
        $out = [];
        foreach (['issueDate' => 'Vystavenie', 'deliveryDate' => 'Dodanie', 'dueDate' => 'Splatnosť'] as $field => $label) {
            $out[$field] = self::date($input[$field] ?? '', $label, $issuing);
        }
        if ($out['dueDate'] !== '' && $out['issueDate'] !== '' && $out['dueDate'] < $out['issueDate']) {
            throw new Problem('Splatnosť nesmie byť pred dátumom vystavenia.');
        }
        $out['orderNumber'] = self::text($input['orderNumber'] ?? '', 80, 'Číslo objednávky');
        $out['note'] = self::text($input['note'] ?? '', 2000, 'Poznámka', false, true);
        $out['currency'] = 'EUR';
        if (($input['currency'] ?? 'EUR') !== 'EUR') {
            throw new Problem('Táto verzia podporuje fakturáciu v EUR.');
        }
        $out['qrProfiles'] = $input['qrProfiles'] ?? ['sk'];
        if (!is_array($out['qrProfiles']) || !array_is_list($out['qrProfiles']) || count($out['qrProfiles']) > 2 || count(array_unique($out['qrProfiles'], SORT_REGULAR)) !== count($out['qrProfiles'])) {
            throw new Problem('Vyber najviac dva rôzne QR kódy.');
        }
        foreach ($out['qrProfiles'] as $profile) {
            if (!is_string($profile) || !isset(self::QR_LABELS[$profile])) {
                throw new Problem('Neznámy typ QR kódu.');
            }
        }
        $out['customerId'] = isset($input['customerId']) ? filter_var($input['customerId'], FILTER_VALIDATE_INT) : null;
        if ($out['customerId'] === false || ($out['customerId'] !== null && $out['customerId'] < 1)) {
            throw new Problem('Neplatný zákazník.');
        }
        if ($issuing && !$out['customerId']) {
            throw new Problem('Vyber odberateľa zo zoznamu zákazníkov.');
        }
        $lines = $input['items'] ?? [];
        if (!is_array($lines) || !array_is_list($lines) || count($lines) < 1 || count($lines) > 100) {
            throw new Problem('Faktúra musí mať 1 až 100 položiek.');
        }
        $out['items'] = [];
        foreach ($lines as $i => $line) {
            if (!is_array($line)) {
                throw new Problem('Neplatná položka faktúry.');
            }
            $description = self::text($line['description'] ?? '', 1000, 'Popis položky ' . ($i + 1), $issuing, true);
            $qty = self::decimal($line['quantity'] ?? '1', 3, 'Množstvo', '1000000');
            $price = self::decimal($line['unitPrice'] ?? '0', 2, 'Jednotková cena', '99999999.99');
            $rate = filter_var($line['vatRate'] ?? 23, FILTER_VALIDATE_INT);
            if ($rate === false || !in_array($rate, [0, 5, 19, 23], true)) {
                throw new Problem('Vyber sadzbu DPH 0, 5, 19 alebo 23 %.');
            }
            if ($issuing && bccomp($qty, '0', 3) <= 0) {
                throw new Problem('Množstvo každej položky musí byť väčšie ako nula.');
            }
            $out['items'][] = ['description' => $description, 'quantity' => $qty, 'unit' => self::text($line['unit'] ?? 'ks', 12, 'Jednotka', $issuing), 'unitPrice' => $price, 'vatRate' => $rate];
        }
        $out['totals'] = self::totals($out['items']);
        if ($issuing && $out['totals']['gross'] <= 0) {
            throw new Problem('Celková suma faktúry musí byť väčšia ako nula.');
        }
        return $out;
    }

    /** @param list<array<string, mixed>> $items @return array<string, mixed> */
    public static function totals(array $items): array
    {
        $net = 0;
        $vat = 0;
        $groups = [];
        $lines = [];
        foreach ($items as $item) {
            $lineNet = (int) bcadd(bcmul(bcmul($item['quantity'], $item['unitPrice'], 5), '100', 5), '0.5', 0);
            $lineVat = (int) bcadd(bcdiv(bcmul((string) $lineNet, (string) $item['vatRate'], 0), '100', 4), '0.5', 0);
            $net += $lineNet;
            $vat += $lineVat;
            if ($net + $vat > 99999999999) {
                throw new Problem('Suma faktúry je príliš vysoká (maximum 999 999 999,99 EUR).');
            }
            $rate = $item['vatRate'];
            $groups[$rate] ??= ['rate' => $rate, 'net' => 0, 'vat' => 0, 'gross' => 0];
            $groups[$rate]['net'] += $lineNet;
            $groups[$rate]['vat'] += $lineVat;
            $groups[$rate]['gross'] += $lineNet + $lineVat;
            $lines[] = ['net' => $lineNet, 'vat' => $lineVat, 'gross' => $lineNet + $lineVat];
        }
        ksort($groups);
        return ['net' => $net, 'vat' => $vat, 'gross' => $net + $vat, 'groups' => array_values($groups), 'lines' => $lines];
    }

    public static function money(int $cents): string
    {
        return number_format($cents / 100, 2, ',', ' ') . ' €';
    }
}
