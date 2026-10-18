# KnowYourEgk

[![CI](https://github.com/frequency403/KnowYourEgk/actions/workflows/ci.yml/badge.svg)](https://github.com/frequency403/KnowYourEgk/actions/workflows/ci.yml)

**See what is stored on your German health insurance card.**

KnowYourEgk is a small desktop app (Windows, Linux, macOS) that reads the
*Versichertenstammdaten* (VSD, insurance master data) from a German
electronic health card (*elektronische Gesundheitskarte*, eGK) of
generation **G2.1**. It works both ways:

- **Contact:** insert the card into a smart card reader.
- **Contactless (NFC):** place the card on an NFC reader and enter the
  six-digit **CAN** printed on the card. The app then sets up an encrypted
  channel with the card (PACE + secure messaging), just like a doctor's
  practice terminal does.

Everything happens locally on your computer. The app has no network code,
stores nothing on disk and never saves the CAN.

> KnowYourEgk is an independent hobby project. It is not affiliated with,
> endorsed or certified by gematik GmbH or any health insurer. It only
> *reads* data and never writes to the card.

## What you get to see

| Section | Contents |
|---|---|
| Personal data (EF.PD) | name, title, date of birth, sex, insurant ID (*Versicherten-ID*), street or postbox address |
| Insurance data (EF.VD) | health insurer and its institution code (IK), billing insurer, coverage start/end, insurant type (member, family member, pensioner), WOP code, special groups / DMP if present |
| Status (EF.StatusVD) | a warning if the last update of the card data did not complete |
| All fields (raw) | every element of the stored XML documents, including fields the app has no label for |

Out of scope: medical data, the emergency data set, protected insurance data
(EF.GVD, which needs a health professional card), PIN-protected objects and
anything that writes to the card.

## Requirements

- An eGK of generation **G2.1**. Contactless reading needs a card with the
  NFC symbol and the CAN printed on the front.
- A PC/SC smart card reader:
  - **Contact:** any standard USB smart card reader (CCID).
  - **Contactless:** an ISO 14443 NFC reader that supports extended-length
    APDUs. Tested with an **HID OMNIKEY 5121** (contact and contactless slot)
    and an **Alcor Micro AU9540** (contact).
- **Linux:** `pcscd` and the CCID driver (packages `pcsc-lite` + `ccid`;
  Debian/Ubuntu: `pcscd libccid`). Make sure the service is running:
  `systemctl enable --now pcscd.socket`.
- **Windows / macOS:** nothing extra, PC/SC is part of the OS. Some
  contactless readers need the vendor driver.

## Installation

### Prebuilt binaries

Every CI run on `main` builds release binaries for Linux, Windows and macOS.
You can download them as artifacts (`knowyouregk-Linux`,
`knowyouregk-Windows`, `knowyouregk-macOS`) from the
[Actions page](https://github.com/frequency403/KnowYourEgk/actions).

> The macOS binary is not signed. On first start, right-click it and choose
> *Open*, or allow it under *System Settings → Privacy & Security*.

### Build from source

You need Rust **1.92** or newer ([rustup.rs](https://rustup.rs)).

```bash
# Linux only: PC/SC headers
sudo apt-get install libpcsclite-dev pkg-config      # Debian/Ubuntu
sudo pacman -S pcsclite ccid                         # Arch

git clone https://github.com/frequency403/KnowYourEgk.git
cd KnowYourEgk
cargo build --release -p kye-app
```

The binary ends up in `target/release/knowyouregk` (`knowyouregk.exe` on
Windows). You can also start it directly with `cargo run --release -p kye-app`.

## Usage

1. Connect your reader(s) and start **KnowYourEgk**. The left panel lists
   every reader slot. A dual-interface reader such as the OMNIKEY 5121 shows
   up as two slots (contact and contactless). Readers can be plugged in and
   out while the app is running.
2. **Contact:** insert the card chip first, as marked on the reader. The slot
   shows *Karte gesteckt (kontaktbehaftet)*. Click **Lesen** (Read).
3. **Contactless:**
   1. Type the **CAN** into the CAN field: the six-digit number on the
      **front** of the card (top right). It is not your PIN.
   2. Place the card flat on the reader and leave it there. The slot shows
      *Karte aufgelegt (kontaktlos)*.
   3. Click **Lesen**. Setting up the encrypted channel takes about
      two seconds. Do not move the card until the data is shown.
4. Your data appears on the right. Expand *Alle Felder (Rohdaten)* to see
   every stored field. **Daten verwerfen** (discard data) clears the view.
   Nothing is kept after you close the app.

The interface is currently in German, as the card and its data are German.

### Messages

| Message | Meaning / what to do |
|---|---|
| *Für kontaktloses Lesen wird die 6-stellige CAN benötigt.* | Enter the CAN before reading contactlessly. |
| *Die CAN ist falsch …* | The card rejected the CAN. Check the number on the front of the card. |
| *Die Karte wurde entfernt oder hat die Verbindung verloren.* | The card was moved or removed during the read. Put it back and click *Lesen* again. |
| *Der Kartenleser ist nicht mehr verfügbar.* | The reader was unplugged. Reconnect it. |
| *Die letzte Aktualisierung der Versichertendaten wurde nicht abgeschlossen.* | The card says its last online update was interrupted, so the data may be incomplete. Your doctor's practice can update the card. |
| *PC/SC nicht verfügbar* | The smart card service is not reachable (Linux: start `pcscd`). |

## Troubleshooting

- **No reader listed (Linux):** run `pcsc_scan` from `pcsc-tools`. If it
  finds no reader either, check `lsusb`, the `ccid` package and
  `systemctl status pcscd`.
- **Contactless card not detected:** place the card centered and flat, and
  keep metal objects and other NFC cards away from the reader.
- **Contactless read fails right away on another reader:** the reader may not
  support extended-length APDUs, which the encrypted channel requires.

## Privacy

- The app has no network code. Card data stays in memory and is dropped when
  you discard it or close the app.
- The CAN and all session keys live in memory only and are zeroized when no
  longer needed.
- Debug logging (`RUST_LOG=kye::apdu=debug`) records only APDU headers,
  lengths and status words, never card content.

## How it works

Rust workspace, card I/O on a dedicated worker thread, UI in
[egui](https://github.com/emilk/egui):

```
crates/
  kye-apdu       ISO 7816-4 command/response APDUs (short + extended), BER-TLV
  kye-transport  CardTransport / ReaderProvider traits, PC/SC backend, mock
  kye-pace       PACE id-PACE-ECDH-GM-AES-CBC-CMAC-128 (brainpoolP256r1), EF.CardAccess
  kye-sm         secure messaging (AES-CBC + CMAC) as a CardTransport decorator
  kye-egk        eGK objects, chunked reads, gzip/ISO-8859-15/XML parsing of PD/VD
  kye-app        desktop app (eframe/egui)
tools/
  m0-spike       hardware diagnostics CLI
```

Contact reads are plain `SELECT` / `READ BINARY`. Contactless access to the
VSD requires PACE with the CAN. All following commands, `SELECT` included,
are wrapped in secure messaging. The implementation follows gemSpec_COS,
gemSpec_eGK_ObjSys_G2.1 and gemSpec_eGK_Fach_VSDM by gematik and ICAO
Doc 9303 Part 11.

### Development

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Unit tests use synthetic data only. PACE is tested against the ICAO 9303-11
worked example, and full card flows against scripted mock transports.
Tests that need a real card are `#[ignore]`d and print no card content:

```bash
# Diagnostics: reader slots, ATRs, contact/contactless checks (+ PACE with KYE_CAN)
KYE_CAN=123456 cargo run -p m0-spike

# Element structure of PD/VD (tag names and text lengths only)
KYE_CAN=123456 cargo test -p kye-egk --test hw_structure -- --ignored --nocapture

# Contact and contactless reads must be byte-identical (asks you to move the card)
KYE_CAN=123456 cargo test -p kye-egk --test hw_oracle -- --ignored --nocapture
```

Replace `123456` with your card's CAN. Never commit real card data or a CAN.

## Status

Tested with a real eGK G2.1 on Linux (pcsc-lite): contact and contactless
reads, wrong CAN, card removal and reader hot-plug. Windows and macOS builds
are produced by CI but have not been tested with a card yet. Reports are
welcome.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option.
