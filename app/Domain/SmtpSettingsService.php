<?php
declare(strict_types=1);
namespace App\Domain;
use App\Infrastructure\Database;

final class SmtpSettingsService
{
    public function __construct(private readonly Database $db) {}
    /** @return array<string, mixed> */
    public function connection(): array
    {
        $row = $this->db->query('SELECT data, version FROM smtp_settings WHERE id=1')->fetch();
        return $row ? json_decode($row['data'], true, 512, JSON_THROW_ON_ERROR) + ['version'=>(int)$row['version']]
            : ['host'=>'','port'=>587,'encryption'=>'tls','username'=>'','password'=>'','fromEmail'=>'','fromName'=>'','replyTo'=>'','version'=>0];
    }
    /** @return array<string, mixed> */
    public function get(): array
    {
        $data = $this->connection();
        $data['passwordSet'] = $data['password'] !== '';
        $data['configured'] = $data['host'] !== '' && $data['fromEmail'] !== '';
        unset($data['password']);
        return $data;
    }
    public static function email(mixed $input, string $label, bool $required = true): string
    {
        $email = Rules::text($input, 160, $label, $required);
        if ($email !== '' && !filter_var($email, FILTER_VALIDATE_EMAIL)) throw new Problem('Skontroluj e-mailovú adresu „' . $label . '“.');
        return $email;
    }
    /** @param array<string, mixed> $input @return array<string, mixed> */
    public function save(array $input): array
    {
        return $this->db->transaction(function () use ($input): array {
            $current = $this->connection();
            if (($input['version'] ?? null) !== $current['version']) throw new Problem('SMTP nastavenia sa zmenili v inom okne. Obnov stránku.', 409);
            $host = Rules::text($input['host'] ?? '', 253, 'SMTP server', true);
            if (!preg_match('/^[a-zA-Z0-9][a-zA-Z0-9.:-]*$/D', $host)) throw new Problem('Zadaj iba názov SMTP servera alebo IP adresu, bez protokolu a cesty.');
            $port = filter_var($input['port'] ?? null, FILTER_VALIDATE_INT);
            if ($port === false || $port < 1 || $port > 65535) throw new Problem('SMTP port musí byť od 1 do 65535.');
            $encryption = $input['encryption'] ?? '';
            if (!in_array($encryption, ['tls','ssl','none'], true)) throw new Problem('Vyber zabezpečenie SMTP spojenia.');
            $username = Rules::text($input['username'] ?? '', 254, 'SMTP používateľ');
            $password = $input['password'] ?? '';
            if (!is_string($password) || strlen($password) > 1024 || preg_match('/[\x00\r\n]/', $password)) throw new Problem('Skontroluj SMTP heslo.');
            $clear = ($input['clearPassword'] ?? false) === true || $current['host'] !== $host || $current['username'] !== $username;
            $data = ['host'=>$host,'port'=>$port,'encryption'=>$encryption,'username'=>$username,
                'password'=>$password !== '' ? $password : ($clear ? '' : $current['password']),
                'fromEmail'=>self::email($input['fromEmail'] ?? '', 'Odosielateľ'),
                'fromName'=>Rules::text($input['fromName'] ?? '',70,'Meno odosielateľa'),
                'replyTo'=>self::email($input['replyTo'] ?? '', 'Odpovede posielať na', false)];
            if ($username !== '' && $data['password'] === '') throw new Problem('Pre prihlásenie zadaj SMTP heslo. Pri zmene servera alebo používateľa ho vyplň znova.');
            if ($current['version'] === 0) $this->db->query('INSERT INTO smtp_settings(id,data,version) VALUES(1,?,1)', [Database::json($data)]);
            else $this->db->query('UPDATE smtp_settings SET data=?, version=version+1 WHERE id=1', [Database::json($data)]);
            return $this->get();
        });
    }
}
