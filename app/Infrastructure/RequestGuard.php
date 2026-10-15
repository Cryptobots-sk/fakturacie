<?php
declare(strict_types=1);
namespace App\Infrastructure;
use App\Domain\Problem;
use Nette\Http\IRequest;
use Nette\Http\Session;
final class RequestGuard
{
    public function __construct(private readonly Session $session, private readonly IRequest $request) {}
    public function token(): string
    {
        $section = $this->session->getSection('requestGuard');
        $token = $section->get('csrf');
        if (!is_string($token)) {
            $token = bin2hex(random_bytes(32));
            $section->set('csrf', $token);
        }
        return $token;
    }
    /** @return array<string, mixed> */
    public function body(): array
    {
        if (!$this->request->isMethod('POST')) {
            throw new Problem('Táto operácia vyžaduje POST.', 405);
        }
        $origin = $this->request->getHeader('Origin');
        if ($origin !== null && $origin !== rtrim($this->request->getUrl()->getHostUrl(), '/')) {
            throw new Problem('Požiadavka z inej stránky bola odmietnutá.', 403);
        }
        if ($this->request->getHeader('Sec-Fetch-Site') === 'cross-site') {
            throw new Problem('Požiadavka z inej stránky bola odmietnutá.', 403);
        }
        $token = $this->request->getHeader('X-CSRF-Token');
        if (!is_string($token) || !hash_equals($this->token(), $token)) {
            throw new Problem('Platnosť stránky vypršala. Obnov ju a skús znovu.', 403);
        }
        if (!str_starts_with(strtolower($this->request->getHeader('Content-Type') ?? ''), 'application/json')) {
            throw new Problem('Očakávam údaje vo formáte JSON.', 415);
        }
        $raw = $this->request->getRawBody() ?? '';
        if (strlen($raw) > 300000) {
            throw new Problem('Požiadavka je príliš veľká.', 413);
        }
        try {
            $data = json_decode($raw, true, 32, JSON_THROW_ON_ERROR);
        } catch (\JsonException) {
            throw new Problem('Údaje sa nepodarilo načítať.');
        }
        if (!is_array($data) || array_is_list($data)) {
            throw new Problem('Neplatná požiadavka.');
        }
        return $data;
    }
}
