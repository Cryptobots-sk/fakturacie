use crate::{
    error::{Problem, Result},
    model::{Invoice, QrImage, string},
    rules,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use image::{DynamicImage, GrayImage, Luma, Rgb, RgbImage};
use imageproc::drawing::{draw_text_mut, text_size};
use qrcode::{EcLevel, QrCode, Version, bits::Bits};
use std::io::{Cursor, Write};

fn uri(image: DynamicImage) -> Result<String> {
    let mut output = Cursor::new(Vec::new());
    image
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(|_| Problem::internal())?;
    Ok(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(output.into_inner())
    ))
}
pub fn placeholder(root: &std::path::Path, label: &str) -> Result<String> {
    let bytes = std::fs::read(root.join("public/assets/fonts/invoice-regular.ttf"))?;
    let font = ab_glyph::FontArc::try_from_vec(bytes).map_err(|_| Problem::internal())?;
    let mut image = RgbImage::from_pixel(408, 408, Rgb([255, 255, 255]));
    for n in 3..=404 {
        for d in 0..2 {
            for (x, y) in [(n, 3 + d), (n, 404 - d), (3 + d, n), (404 - d, n)] {
                image.put_pixel(x, y, Rgb([188, 203, 213]));
            }
        }
    }
    let scale = ab_glyph::PxScale::from(29.333);
    let (width, height) = text_size(scale, &font, label);
    draw_text_mut(
        &mut image,
        Rgb([82, 103, 121]),
        ((408 - width) / 2) as i32,
        214 - height as i32,
        scale,
        &font,
        label,
    );
    uri(image.into())
}
fn raw_urlencode(text: &str) -> String {
    percent_encoding::utf8_percent_encode(text, percent_encoding::NON_ALPHANUMERIC)
        .to_string()
        .replace("%2D", "-")
        .replace("%5F", "_")
        .replace("%2E", ".")
        .replace("%7E", "~")
}
fn base32hex(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUV";
    let mut output = String::new();
    let mut buffer = 0_u32;
    let mut bits = 0;
    for byte in data {
        buffer = (buffer << 8) | u32::from(*byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            output.push(ALPHABET[((buffer >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        output.push(ALPHABET[((buffer << (5 - bits)) & 31) as usize] as char);
    }
    output
}
pub fn payload(invoice: &Invoice, profile: &str) -> Result<String> {
    let supplier = &invoice.data.supplier;
    let amount = format!(
        "{}.{:02}",
        invoice.data.totals.amounts.gross / 100,
        invoice.data.totals.amounts.gross % 100
    );
    let number = invoice
        .number
        .as_deref()
        .ok_or_else(|| Problem::new("Faktúra ešte nemá číslo."))?;
    let iban = string(supplier, "iban");
    let bic = string(supplier, "bic");
    let name = string(supplier, "name");
    match profile {
        "sk" => {
            if invoice.data.totals.amounts.gross <= 0 || !rules::valid_iban(iban) {
                return Err(Problem::new("Skontroluj sumu a IBAN pre platobný QR kód."));
            }
            let note = format!("Faktura {number}");
            let data = [
                "", "1", "1", &amount, "EUR", "", number, "", "", "", &note, "1", iban, bic, "0",
                "0", name, "", "",
            ]
            .join("\t");
            let mut checked = crc32fast::hash(data.as_bytes()).to_le_bytes().to_vec();
            checked.extend_from_slice(data.as_bytes());
            let mut options =
                xz2::stream::LzmaOptions::new_preset(6).map_err(|_| Problem::internal())?;
            options
                .dict_size(131072)
                .literal_context_bits(3)
                .literal_position_bits(0)
                .position_bits(2);
            let stream =
                xz2::stream::Stream::new_lzma_encoder(&options).map_err(|_| Problem::internal())?;
            let mut encoder = xz2::write::XzEncoder::new_stream(Vec::new(), stream);
            encoder.write_all(&checked)?;
            let compressed = encoder.finish()?;
            if compressed.len() < 13 {
                return Err(Problem::internal());
            }
            let length = u16::try_from(checked.len()).map_err(|_| Problem::internal())?;
            let mut bytes = vec![0, 0];
            bytes.extend_from_slice(&length.to_le_bytes());
            bytes.extend_from_slice(&compressed[13..]);
            Ok(base32hex(&bytes))
        }
        "cz" => {
            if amount.len() > 10 {
                return Err(Problem::new(
                    "QR Platba podporuje sumu najviac 9 999 999,99 EUR. Vyber iný QR formát.",
                ));
            }
            let account = if bic.is_empty() {
                iban.to_owned()
            } else {
                format!("{iban}+{bic}")
            };
            let name: String = name.chars().take(35).collect();
            Ok(format!(
                "SPD*1.0*ACC:{account}*AM:{amount}*CC:EUR*RN:{}*MSG:FAKTURA%20{number}*X-VS:{number}",
                raw_urlencode(&name)
            ))
        }
        "epc" => {
            let note = format!("Faktura {number}");
            let payload = [
                "BCD",
                "002",
                "1",
                "SCT",
                bic,
                name,
                iban,
                &format!("EUR{amount}"),
                "",
                "",
                &note,
            ]
            .join("\n");
            if payload.len() > 331 {
                return Err(Problem::new(
                    "Údaje sú príliš dlhé pre SEPA QR. Skráť názov dodávateľa alebo vyber iný QR formát.",
                ));
            }
            Ok(payload)
        }
        _ => Err(Problem::new("Neznámy typ platobného QR kódu.")),
    }
}
pub fn images(invoice: &Invoice) -> Result<Vec<QrImage>> {
    invoice
        .data
        .qr_profiles
        .iter()
        .map(|profile| {
            let payload = payload(invoice, profile)?;
            let code = if profile == "epc" {
                // EPC declares UTF-8 and its QR must retain the same explicit ECI as the PHP encoder.
                (1..=40)
                    .find_map(|version| {
                        let mut bits = Bits::new(Version::Normal(version));
                        bits.push_eci_designator(26).ok()?;
                        bits.push_byte_data(payload.as_bytes()).ok()?;
                        bits.push_terminator(EcLevel::M).ok()?;
                        QrCode::with_bits(bits, EcLevel::M).ok()
                    })
                    .ok_or_else(Problem::internal)?
            } else {
                QrCode::with_error_correction_level(
                    payload.as_bytes(),
                    if profile == "sk" {
                        EcLevel::L
                    } else {
                        EcLevel::M
                    },
                )
                .map_err(|_| Problem::internal())?
            };
            let width = code.width();
            let mut image = GrayImage::from_pixel(408, 408, Luma([255]));
            for y in 0..360_u32 {
                for x in 0..360_u32 {
                    if code[(x as usize * width / 360, y as usize * width / 360)]
                        == qrcode::Color::Dark
                    {
                        image.put_pixel(x + 24, y + 24, Luma([0]));
                    }
                }
            }
            Ok(QrImage {
                profile: profile.clone(),
                label: rules::label(profile)
                    .ok_or_else(Problem::internal)?
                    .to_owned(),
                data_uri: uri(image.into())?,
            })
        })
        .collect()
}
