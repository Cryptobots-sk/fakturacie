import * as sass from 'sass';
import * as esbuild from 'esbuild';
import {writeFile, mkdir, readFile, copyFile} from 'node:fs/promises';
await mkdir('public/assets',{recursive:true});
await mkdir('public/assets/fonts',{recursive:true});
for (const target of ['invoice-regular.ttf','invoice-bold.ttf','LICENSE.txt']) {
  await copyFile(`assets/fonts/${target}`,`public/assets/fonts/${target}`);
}
const css=sass.compile('assets/scss/app.scss',{loadPaths:['node_modules'],style:'compressed',quietDeps:true,silenceDeprecations:['import','global-builtin','color-functions']});
await writeFile('public/assets/app.css',css.css + '\n' + await readFile('assets/invoice.css','utf8'));
await esbuild.build({entryPoints:['assets/js/app.js'],outfile:'public/assets/app.js',bundle:true,minify:true,target:['safari17','chrome120','firefox120']});
console.log('Vzhľad a editor sú zostavené.');
