<?php
declare(strict_types=1);
namespace App;
use Nette\Bootstrap\Configurator;
use Nette\DI\Container;
final class Bootstrap
{
    public static function boot(): Container
    {
        $root = dirname(__DIR__);
        date_default_timezone_set('Europe/Bratislava');
        $configurator = new Configurator;
        $configurator->setDebugMode(false);
        $configurator->enableTracy($root . '/log');
        $configurator->setTempDirectory($root . '/temp');
        $configurator->addStaticParameters(['appDir' => $root . '/app', 'wwwDir' => $root . '/public', 'rootDir' => $root, 'databasePath' => getenv('FAKTURACIE_DB') ?: $root . '/data/fakturacie.sqlite']);
        $configurator->addConfig($root . '/config/common.neon');
        return $configurator->createContainer();
    }
}
