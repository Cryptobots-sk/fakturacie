export function decimalUnits(value, scale) {
  const normalized = String(value).trim().replace(/[ \u00a0]/g, '').replace(',', '.') || '0';
  if (!new RegExp(`^\\d{1,9}(\\.\\d{1,${scale}})?$`).test(normalized)) throw new Error('Neplatné číslo');
  const [whole, fraction = ''] = normalized.split('.');
  return BigInt(whole) * (10n ** BigInt(scale)) + BigInt(fraction.padEnd(scale, '0'));
}
export function calculate(items) {
  let net = 0n, vat = 0n;
  const groups = new Map();
  const lines = items.map(item => {
    const quantity = decimalUnits(item.quantity, 3);
    const price = decimalUnits(item.unitPrice, 2);
    const rate = BigInt(item.vatRate);
    const lineNet = (quantity * price + 500n) / 1000n;
    const lineVat = (lineNet * rate + 50n) / 100n;
    net += lineNet; vat += lineVat;
    const group = groups.get(item.vatRate) || {rate: item.vatRate, net: 0, vat: 0, gross: 0};
    group.net += Number(lineNet); group.vat += Number(lineVat); group.gross += Number(lineNet + lineVat);
    groups.set(item.vatRate, group);
    return {net: Number(lineNet), vat: Number(lineVat), gross: Number(lineNet + lineVat)};
  });
  return {net: Number(net), vat: Number(vat), gross: Number(net + vat), lines, groups: [...groups.values()].sort((a,b) => a.rate-b.rate)};
}
export const money = cents => new Intl.NumberFormat('sk-SK', {style:'currency', currency:'EUR'}).format(cents / 100);
