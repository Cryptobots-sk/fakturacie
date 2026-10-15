<?php
declare(strict_types=1);
namespace App\Domain;
use Endroid\QrCode\QrCode;
use Endroid\QrCode\Encoding\Encoding;
use Endroid\QrCode\ErrorCorrectionLevel;
use Endroid\QrCode\Writer\PngWriter;
use PayBySquare\Payment;
use PayBySquare\Base32Hex;
use PayBySquare\Lzma\Lzma1Encoder;

final class QrService
{
    public static function placeholder(string $label = 'QR po vystavení'): string
    {
        $image = imagecreatetruecolor(408, 408);
        $white = imagecolorallocate($image, 255, 255, 255);
        $border = imagecolorallocate($image, 188, 203, 213);
        $text = imagecolorallocate($image, 82, 103, 121);
        imagefill($image, 0, 0, $white);
        imagerectangle($image, 3, 3, 404, 404, $border);
        $font = dirname(__DIR__, 2) . '/public/assets/fonts/invoice-regular.ttf';
        $box = imagettfbbox(22, 0, $font, $label);
        imagettftext($image, 22, 0, (int) ((408 - ($box[2] - $box[0])) / 2), 214, $text, $font, $label);
        ob_start();
        imagepng($image);
        $png = (string) ob_get_clean();
        imagedestroy($image);
        return 'data:image/png;base64,' . base64_encode($png);
    }
    /** @param array<string, mixed> $invoice */
    public function payload(array $invoice, string $profile): string
    {
        $supplier = $invoice['supplier'];
        $amount = bcdiv((string) $invoice['totals']['gross'], '100', 2);
        $number = $invoice['number'];
        $iban = $supplier['iban'];
        $bic = $supplier['bic'];
        $name = $supplier['name'];
        if ($profile === 'sk') {
            $payment = new Payment(amount: (float) $amount, iban: $iban, beneficiaryName: $name, swift: $bic, variableSymbol: $number, note: 'Faktura ' . $number);
            $fields = explode("\t", $payment->toDataString());
            // Optional payment date is intentionally empty: an overdue invoice stays payable.
            $fields[5] = '';
            $data = implode("\t", $fields);
            $checked = pack('V', crc32($data)) . $data;
            return Base32Hex::encode("\0\0" . pack('v', strlen($checked)) . Lzma1Encoder::compress($checked));
        }
        if ($profile === 'cz') {
            if (strlen($amount) > 10) {
                throw new Problem('QR Platba podporuje sumu najviac 9 999 999,99 EUR. Vyber iný QR formát.');
            }
            return 'SPD*1.0*ACC:' . $iban . ($bic !== '' ? '+' . $bic : '') . '*AM:' . $amount . '*CC:EUR*RN:' . rawurlencode(mb_substr($name, 0, 35)) . '*MSG:FAKTURA%20' . $number . '*X-VS:' . $number;
        }
        if ($profile === 'epc') {
            $payload = implode("\n", ['BCD', '002', '1', 'SCT', $bic, $name, $iban, 'EUR' . $amount, '', '', 'Faktura ' . $number]);
            if (strlen($payload) > 331) {
                throw new Problem('Údaje sú príliš dlhé pre SEPA QR. Skráť názov dodávateľa alebo vyber iný QR formát.');
            }
            return $payload;
        }
        throw new Problem('Neznámy typ platobného QR kódu.');
    }
    /** @param array<string, mixed> $invoice @return list<array<string, string>> */
    public function images(array $invoice): array
    {
        $images = [];
        foreach ($invoice['qrProfiles'] as $profile) {
            $payload = $this->payload($invoice, $profile);
            $qr = new QrCode(data: $payload, encoding: new Encoding($profile === 'epc' ? 'UTF-8' : 'ISO-8859-1'), errorCorrectionLevel: $profile === 'sk' ? ErrorCorrectionLevel::Low : ErrorCorrectionLevel::Medium, size: 360, margin: 24);
            $images[] = ['profile' => $profile, 'label' => Rules::QR_LABELS[$profile], 'dataUri' => (new PngWriter())->write($qr)->getDataUri()];
        }
        return $images;
    }
}
