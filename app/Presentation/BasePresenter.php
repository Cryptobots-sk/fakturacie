<?php
declare(strict_types=1);
namespace App\Presentation;
use App\Domain\Rules;
use App\Infrastructure\RequestGuard;
use Nette\Application\UI\Presenter;
use Nette\Application\UI\Template;
/** @property-read AppTemplate $template */
abstract class BasePresenter extends Presenter
{
    private RequestGuard $guard;
    public function injectGuard(RequestGuard $guard): void { $this->guard = $guard; }
    protected function createTemplate(?string $class = null): Template { return parent::createTemplate($class ?? AppTemplate::class); }
    protected function beforeRender(): void
    {
        parent::beforeRender();
        $this->template->csrf = $this->guard->token();
        $this->template->getLatte()->addFilter('money', Rules::money(...));
    }
    /** @param array<string, mixed> $data */
    protected function bootData(array $data): void
    {
        $this->template->bootJson = json_encode($data, JSON_THROW_ON_ERROR | JSON_HEX_TAG | JSON_HEX_AMP | JSON_HEX_APOS | JSON_HEX_QUOT | JSON_UNESCAPED_UNICODE);
    }
}
