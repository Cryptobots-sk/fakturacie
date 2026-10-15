<?php
declare(strict_types=1);
$path = dirname(__DIR__) . '/temp/cache';
if (is_dir($path)) {
    $files = new RecursiveIteratorIterator(new RecursiveDirectoryIterator($path, FilesystemIterator::SKIP_DOTS), RecursiveIteratorIterator::CHILD_FIRST);
    foreach ($files as $file) { if ($file->isDir() && !$file->isLink()) rmdir($file->getPathname()); else unlink($file->getPathname()); }
}
echo "Cache šablón a kontajnera je vyčistená.\n";
