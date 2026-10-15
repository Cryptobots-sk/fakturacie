import test from 'node:test';
import assert from 'node:assert/strict';
import {calculate, decimalUnits} from '../assets/js/money.mjs';
test('Reference invoice is 3542.40 EUR',()=>{const totals=calculate([{quantity:'1',unitPrice:'2880,00',vatRate:23}]);assert.equal(totals.net,288000);assert.equal(totals.vat,66240);assert.equal(totals.gross,354240);});
test('Half cents and mixed VAT stay exact',()=>{const totals=calculate([{quantity:'0.125',unitPrice:'0.04',vatRate:23},{quantity:'3',unitPrice:'0.10',vatRate:19}]);assert.equal(totals.net,31);assert.equal(totals.vat,6);assert.equal(totals.gross,37);});
test('Comma and grouping accepted; expressions and excessive precision rejected',()=>{assert.equal(decimalUnits('1 234,50',2),123450n);for(const value of ['1e3','Infinity','-1','1.234','<script>']) assert.throws(()=>decimalUnits(value,2));});
