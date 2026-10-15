<?php
declare(strict_types=1);
namespace App\Infrastructure;
use Nette\Application\Routers\RouteList;
final class RouterFactory
{
    public static function create(): RouteList
    {
        $router = new RouteList;
        $router->addRoute('api/<action>', 'Api:default');
        $router->addRoute('faktury/<id \d+>/pdf', 'Invoice:pdf');
        $router->addRoute('faktury/<id \d+>', 'Invoice:default');
        $router->addRoute('faktury/nova', 'Invoice:default');
        $router->addRoute('zakaznici', 'Customers:default');
        $router->addRoute('nastavenia', 'Settings:default');
        $router->addRoute('', 'Home:default');
        return $router;
    }
}
