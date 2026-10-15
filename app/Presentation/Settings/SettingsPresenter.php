<?php
declare(strict_types=1);
namespace App\Presentation\Settings;
use App\Domain\{SettingsService, SmtpSettingsService};
use App\Presentation\BasePresenter;
final class SettingsPresenter extends BasePresenter
{
    public function __construct(private readonly SettingsService $settings, private readonly SmtpSettingsService $smtp) { parent::__construct(); }
    public function renderDefault(): void
    {
        $this->template->title = 'Moja firma';
        $this->template->page = 'settings';
        $this->template->supplier = $this->settings->get();
        $this->bootData(['page' => 'settings', 'supplier' => $this->template->supplier, 'smtp'=>$this->smtp->get(), 'smtpTestKey'=>bin2hex(random_bytes(16))]);
    }
}
