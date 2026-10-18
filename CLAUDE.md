# KnowYourEgk – Agent Brief

Rust/egui-Desktop-App, die die Versichertenstammdaten (VSD) einer eGK G2.1 ausliest – **gesteckt** (kontaktbehaftet, ohne Krypto) oder **aufgelegt** (kontaktlos, PACE-CAN + Secure Messaging).

- Plattformen jetzt: Windows, Linux, macOS. Android/iOS folgen später → Transport strikt abstrahieren.
- Scope: **nur VSD** (EF.StatusVD, EF.PD, EF.VD). Kein EF.GVD (erfordert C2C mit HBA/SMC-B), keine PIN, kein Schreiben.
- Testhardware: eine echte eGK (G2.1, CAN aufgedruckt), Omnikey Dual-Interface (Kontakt- + CL-Slot). Keine gematik-Testkarten.

## Konventionen

- Code-Kommentare auf Englisch, Rust-Standardkonventionen (`rustfmt`, `clippy -D warnings`).
- Keine `unwrap()`/`expect()` außerhalb von Tests. Fehler über `thiserror`.
- Schlüsselmaterial (K_π, K_enc, K_mac, CAN) mit `zeroize`. CAN niemals persistieren.

## Workspace

```
crates/
  kye-apdu       # Command/Response APDU (short + extended), BER-TLV
  kye-transport  # CardTransport + ReaderProvider traits; backend "pcsc" (feature-gated)
  kye-sm         # SecureChannel<T: CardTransport>, itself a CardTransport (decorator)
  kye-pace       # PACE ECDH-GM, EF.CardAccess (SecurityInfos) parsing
  kye-egk        # eGK object model, chunked reads, VSD parsing
  kye-app        # eframe/egui; card I/O on a worker thread, UI via channels
tools/
  m0-spike       # M0 hardware spike (CLI), see "Befehle"
```

Kartenzugriff blockierend im Worker-Thread, kein async.

## Befehle

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p m0-spike            # needs pcscd (Linux) and a reader; RUST_LOG=debug for APDU trace
```

## Normative Quellen

- gemSpec_eGK_ObjSys_G2.1 – Zugriffsregeln, FIDs/SFIs
- gemSpec_COS – PACE (N085.064), MSE (CosT_a40), GA (CosT_a9a/19a/29a/152), SM (Kap. 13.2/13.3), KDF (N001.530)
- gemSpec_eGK_Fach_VSDM – Dateiformate
- Testvektoren: ICAO Doc 9303 Part 11, Worked Example PACE ECDH-GM brainpoolP256r1 AES-128 (Passwort dort MRZ statt CAN)

## Objekte & Zugriff

| Objekt | FID / SFI | Kontakt | Kontaktlos |
|---|---|---|---|
| EF.CardAccess (MF) | 011C / 1C | ALWAYS | ALWAYS |
| DF.HCA | AID `D2 76 00 00 01 02` | SELECT ALWAYS | SELECT ALWAYS |
| EF.StatusVD | D00C / 0C | ALWAYS | AUT_PACE |
| EF.PD | D001 / 01 | ALWAYS | AUT_PACE |
| EF.VD | D002 / 02 | ALWAYS | AUT_PACE |

`AUT_PACE = SmMac(SK.CAN) AND SmCmdEnc AND SmRspEnc` – Antworten kommen immer verschlüsselt.

## Dateiformate (gzip-XML, Zeichensatz ISO-8859-15)

- **EF.PD:** `len(2, BE) || gzip(XML)`
- **EF.VD:** `offStartVD(2) || offEndVD(2) || offStartGVD(2) || offEndGVD(2) || …`. GVD-Offsets `FFFF` = keine GVD-Kopie. Die GVD-Kopie wird ignoriert.
- **EF.StatusVD (25 Byte):** `status(1, ASCII '0'/'1') || timestamp(14, UTC YYYYMMDDhhmmss) || versionXml(5, BCD) || versionStorage(5, BCD)`. Status `'1'` = Update unvollständig → in der UI warnen.
- Crates: `flate2`, `encoding_rs` (ISO-8859-15), `quick-xml` (namespace-agnostisch parsen, Schemaversion aus StatusVD).

## Lesen

- Erst SELECT DF.HCA, dann READ BINARY. Der erste Chunk geht per SFI (`P1 = 0x80 | sfi`, P2 = Offset ≤ 255), danach per 15-Bit-Offset in P1/P2 auf dem implizit selektierten EF.
- Länge aus Header lesen (PD-Länge bzw. VD-Offsets), nicht bis EOF lesen.

## Interface-Erkennung

- Primär über ATR: Der PC/SC-Pseudo-ATR `3B 8x 80 01 …` bedeutet kontaktlos. Der Kontakt-ATR der eGK ist `3B 9x/Dx … 81 B1 FE 45 1F …`.
- Nicht über Reader-Namen erkennen (OS-abhängig). Beide Omnikey-Slots parallel überwachen.
- Fallback: Plain-READ auf EF.StatusVD liefert `69 82` → PACE durchführen.

## PACE (id-PACE-ECDH-GM-AES-CBC-CMAC-128, brainpoolP256r1)

```
00 22 C1 A4 0F 80 0A 04007F00070202040202 83 01 02   MSE:Set AT, keyRef 02 = CAN
10 86 00 00 02 7C 00 00                               GA1 -> 7C 12 80 10 z
10 86 00 00 45 7C 43 81 41 <PK1_PCD> 00               GA2 -> 7C .. 82 41 <PK1_PICC>
10 86 00 00 45 7C 43 83 41 <PK2_PCD> 00               GA3 -> 7C .. 84 41 <PK2_PICC>
00 86 00 00 0C 7C 0A 85 08 <T_PCD> 00                 GA4 -> 7C 0A 86 08 <T_PICC>
```

- `KDF(kd, n) = SHA-1(kd || be32(n))[0..16]`. Daraus K_π = KDF(CAN_ASCII, 3), K_enc = KDF(KD, 1), K_mac = KDF(KD, 2).
- `s = AES-DEC(K_π, z)` (ein Block). `G~ = s·G + ECDH(SK1_PCD, PK1_PICC)`.
- KD = x-Koordinate von `SK2_PCD · PK2_PICC`.
- `T_PCD = CMAC(K_mac, 7F49 L [06 0A OID || 86 41 PK2_PICC])[0..8]` ohne Padding. T_PICC analog über PK2_PCD prüfen und bei Abweichung abbrechen.
- Algorithmus/Parameter aus EF.CardAccess (PACEInfo) lesen, nicht hart verdrahten.
- Crypto: RustCrypto (`aes`, `cbc`, `cmac`, `sha1`, `bp256` mit `arithmetic`).

## Secure Messaging

- SSC startet bei 0, `+1` vor jedem Command **und** vor jeder Response.
- `IV = AES-ENC(K_enc, SSC_16)`, AES-CBC, ISO-Padding. Datenobjekte: `87 L 01 C`, Le → `97 01 00`.
- `MAC = CMAC(K_mac, Pad(SSC || Pad(CLA'‖INS‖P1‖P2) || DOs))[0..8]`. ISO-Padding wird **vor** dem CMAC angewendet. CLA' = CLA | 0x0C. Der Header wird nicht gepaddet, wenn keine DOs folgen.
- **Gesicherte Case-2/4-Commands (READ BINARY) als Case 4 Extended mit Le `00 00`.** Case 1/3 → Case 4 Short.
- Response: `[87|81] || 99 02 SW || 8E 08 MAC`. MAC über `SSC || 87/81 || 99` prüfen, dann entschlüsseln.
- Nach PACE **alle** APDUs über SM senden, auch SELECT. (Annahme; das Verhalten bei Plain-APDUs ist in der Spec nicht eindeutig geklärt.)

## Plattform-Fallstricke

- PACE + Lesen in `SCardBeginTransaction`/`EndTransaction` mit `ShareMode::Exclusive` kapseln, weil Windows Smart Card PnP Fremd-APDUs schickt.
- Extended APDU auf dem CL-Slot ist für das konkrete Modell unverifiziert → im Spike testen.

## Datenschutz

- `TracingTransport`-Decorator loggt standardmäßig nur Header, Lc/Le und SW. Response-Daten nur hinter dem Feature-Flag `trace-payload`.
- Keine echten Kartendaten in Repo, Tests oder Fixtures. Fixtures synthetisch erzeugen (gzip-XML selbst bauen).

## Teststrategie

- Unit-Tests: KDF, CMAC, SM-Wrap/Unwrap, PACE-Mapping gegen ICAO-Vektoren, Parser mit synthetischen Fixtures.
- `MockTransport` mit Replay-Skripten für Flows und Fehlerpfade (`69 82`, `63 Cx`, Karte entfernt, falsche CAN).
- **Hardware-Orakel:** Die Rohbytes von EF.StatusVD/PD/VD müssen kontaktbehaftet und kontaktlos byte-identisch sein (`#[ignore]`-Test, manuell mit Karte).

## Meilensteine

| | Inhalt | Abnahme |
|---|---|---|
| M0 | Spike: Reader-Enum beider Slots, ATR, Kontakt-Read StatusVD; CL: CardAccess, MSE, GA1, Extended-APDU-Test | `90 00` + 25 Byte; GA1 → `7C 12 80 10 …` |
| M1 | Kontaktpfad komplett, Parser, egui-MVP | Daten korrekt angezeigt |
| M2 | PACE | T_PICC verifiziert |
| M3 | SM-Lesen kontaktlos | Byte-Vergleich mit Kontaktpfad grün |
| M4 | Fehlerpfade, Hot-Plug, CI-Matrix Win/Linux/macOS | Builds + manuelle Tests auf allen OS |
