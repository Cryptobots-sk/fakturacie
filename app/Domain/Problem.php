<?php
declare(strict_types=1);
namespace App\Domain;
final class Problem extends \RuntimeException
{
    /** @param array<string, string> $fields */
    public function __construct(string $message, public readonly int $status = 422, public readonly array $fields = [])
    {
        parent::__construct($message);
    }
}
