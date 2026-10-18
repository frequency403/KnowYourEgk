//! M0 hardware spike.
//!
//! Enumerates all PC/SC reader slots and runs the M0 checks on every card:
//! - contact: SELECT DF.HCA, READ BINARY EF.StatusVD -> `90 00` + 25 bytes
//! - contactless: EF.CardAccess, plain StatusVD (expect `69 82`), MSE:Set AT,
//!   GA1 -> `7C 12 80 10 ..`, extended APDUs (case 2E and 4E)
//! - contactless, if `KYE_CAN` is set: full PACE run, T_PICC verified (M2)
//!
//! Prints only non-personal data (ATR, EF.CardAccess, StatusVD metadata).
//! Set `RUST_LOG=kye::apdu=debug` to trace APDU headers and status words.

use std::process::ExitCode;

use kye_apdu::{Command, Encoding, Hex, MAX_NE_EXTENDED, MAX_NE_SHORT, StatusWord};
use kye_egk::StatusVd;
use kye_egk::commands::{read_binary, read_binary_sfi, select_df_hca, select_mf};
use kye_egk::objects::{EF_CARD_ACCESS, EF_STATUS_VD};
use kye_egk::status_vd::STATUS_VD_LEN;
use kye_pace::apdu::{PasswordRef, ga_get_nonce, mse_set_at_pace, parse_encrypted_nonce};
use kye_pace::{CardAccess, PaceInfo, PaceProtocol, Password, StandardizedDomainParameters};
use kye_transport::pcsc::PcscProvider;
use kye_transport::{CardInterface, CardTransport, ReaderProvider, TracingTransport};

type BoxError = Box<dyn std::error::Error>;

#[derive(Default)]
struct Report {
    checks: Vec<(String, Result<String, String>)>,
}

impl Report {
    fn pass(&mut self, name: impl Into<String>, detail: impl Into<String>) {
        let (name, detail) = (name.into(), detail.into());
        println!("  [PASS] {name}: {detail}");
        self.checks.push((name, Ok(detail)));
    }

    fn fail(&mut self, name: impl Into<String>, detail: impl Into<String>) {
        let (name, detail) = (name.into(), detail.into());
        println!("  [FAIL] {name}: {detail}");
        self.checks.push((name, Err(detail)));
    }

    fn record<T>(&mut self, name: &str, r: Result<T, BoxError>) -> Option<T> {
        match r {
            Ok(v) => Some(v),
            Err(e) => {
                self.fail(name, e.to_string());
                None
            }
        }
    }
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_target(false)
        .init();

    let mut report = Report::default();
    if let Err(e) = run(&mut report) {
        report.fail("spike", e.to_string());
    }

    let failed = report.checks.iter().filter(|(_, r)| r.is_err()).count();
    println!(
        "\nSummary: {} passed, {failed} failed",
        report.checks.len() - failed
    );
    if failed == 0 && !report.checks.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn run(report: &mut Report) -> Result<(), BoxError> {
    let provider = PcscProvider::new()?;
    let readers = provider.readers()?;
    if readers.is_empty() {
        return Err("no PC/SC readers found (is pcscd running?)".into());
    }

    println!("Readers:");
    for r in &readers {
        match &r.atr {
            Some(atr) => println!(
                "  {} -> {:?}, ATR {}",
                r.name,
                CardInterface::from_atr(atr),
                Hex(atr)
            ),
            None => println!("  {} -> empty", r.name),
        }
    }
    report.pass("reader enumeration", format!("{} slot(s)", readers.len()));

    let mut seen = (false, false);
    for r in readers.iter().filter(|r| r.atr.is_some()) {
        println!();
        match r.interface() {
            Some(CardInterface::Contact) => {
                println!("== Contact: {}", r.name);
                seen.0 = true;
                let res = contact(&provider, &r.name, report);
                report.record("contact flow", res);
            }
            Some(CardInterface::Contactless) => {
                println!("== Contactless: {}", r.name);
                seen.1 = true;
                let res = contactless(&provider, &r.name, report);
                report.record("contactless flow", res);
            }
            _ => println!("== {}: unknown interface, skipped", r.name),
        }
    }

    if !seen.0 {
        println!("\nNote: no card in a contact slot, contact checks skipped.");
    }
    if !seen.1 {
        println!("Note: no card on a contactless slot, contactless checks skipped.");
    }
    Ok(())
}

fn contact(provider: &PcscProvider, reader: &str, report: &mut Report) -> Result<(), BoxError> {
    let mut card = provider.connect(reader)?;
    let mut tx = card.transaction()?;
    {
        let mut t = TracingTransport::new(&mut tx, "contact");
        expect_ok(&mut t, "SELECT DF.HCA", &select_df_hca())?;

        let rsp = t.transmit(&read_binary_sfi(EF_STATUS_VD.sfi, 0, STATUS_VD_LEN))?;
        if rsp.sw.is_ok() && rsp.data.len() == STATUS_VD_LEN {
            report.pass(
                "contact READ EF.StatusVD",
                format!("{} + {} bytes", rsp.sw, rsp.data.len()),
            );
            print_status_vd(&rsp.data, report);
        } else {
            report.fail(
                "contact READ EF.StatusVD",
                format!("SW {} with {} bytes", rsp.sw, rsp.data.len()),
            );
        }

        // EF.CardAccess is readable over contact as well; useful for comparison.
        expect_ok(&mut t, "SELECT MF", &select_mf())?;
        let ca = read_card_access(&mut t)?;
        println!("  EF.CardAccess ({} bytes): {}", ca.len(), Hex(&ca));
    }
    tx.end(true)?;
    card.disconnect_reset()?;
    Ok(())
}

fn contactless(provider: &PcscProvider, reader: &str, report: &mut Report) -> Result<(), BoxError> {
    let mut card = provider.connect(reader)?;
    let mut tx = card.transaction()?;
    {
        let mut t = TracingTransport::new(&mut tx, "cl");

        // 1. EF.CardAccess (MF is current after reset).
        let raw = read_card_access(&mut t)?;
        println!("  EF.CardAccess ({} bytes): {}", raw.len(), Hex(&raw));
        let ca = CardAccess::parse(&raw)?;
        for info in &ca.infos {
            println!("    {info:?}");
        }
        report.pass("CL READ EF.CardAccess", format!("{} bytes", raw.len()));
        let pace = select_pace_info(&ca, report)?;

        // 2. Extended length, case 2E: READ BINARY EF.CardAccess with Le 00 00 00.
        let ext = read_binary_sfi(EF_CARD_ACCESS.sfi, 0, MAX_NE_EXTENDED)
            .with_encoding(Encoding::Extended);
        match t.transmit(&ext) {
            Ok(rsp) if (rsp.sw.is_ok() || rsp.sw == StatusWord::END_OF_FILE) && rsp.data == raw => {
                report.pass(
                    "CL extended APDU (case 2E)",
                    format!("SW {}, {} bytes identical", rsp.sw, rsp.data.len()),
                )
            }
            Ok(rsp) => report.fail(
                "CL extended APDU (case 2E)",
                format!(
                    "SW {}, {} bytes, content differs or error",
                    rsp.sw,
                    rsp.data.len()
                ),
            ),
            Err(e) => report.fail("CL extended APDU (case 2E)", e.to_string()),
        }

        // 3. Plain read of EF.StatusVD must be refused contactless (AUT_PACE).
        expect_ok(&mut t, "SELECT DF.HCA", &select_df_hca())?;
        let rsp = t.transmit(&read_binary_sfi(EF_STATUS_VD.sfi, 0, STATUS_VD_LEN))?;
        if rsp.sw == StatusWord::SECURITY_STATUS_NOT_SATISFIED {
            report.pass("CL plain READ EF.StatusVD refused", rsp.sw.to_string());
        } else {
            report.fail(
                "CL plain READ EF.StatusVD refused",
                format!(
                    "expected 69 82, got {} with {} bytes",
                    rsp.sw,
                    rsp.data.len()
                ),
            );
        }
        expect_ok(&mut t, "SELECT MF", &select_mf())?;

        // 4. MSE:Set AT + GA1 (short).
        let mse = mse_set_at_pace(&pace.oid, PasswordRef::Can, None);
        let rsp = t.transmit(&mse)?;
        if rsp.sw.is_ok() {
            report.pass("CL MSE:Set AT", rsp.sw.to_string());
        } else {
            report.fail("CL MSE:Set AT", rsp.sw.to_string());
            return Ok(());
        }
        let rsp = t.transmit(&ga_get_nonce())?;
        let header_ok = rsp.data.starts_with(&[0x7C, 0x12, 0x80, 0x10]);
        match parse_encrypted_nonce(&rsp) {
            Ok(z) if header_ok => report.pass(
                "CL GA1",
                format!(
                    "7C 12 80 10 + {}-byte encrypted nonce, SW {}",
                    z.len(),
                    rsp.sw
                ),
            ),
            Ok(z) => report.fail(
                "CL GA1",
                format!("unexpected framing, nonce {} bytes", z.len()),
            ),
            Err(e) => report.fail("CL GA1", e.to_string()),
        }

        // 5. Extended length, case 4E: repeat MSE + GA1 with extended length fields.
        //    Secured READ BINARY will use case 4E, so this must work on the CL slot.
        let rsp = t.transmit(&mse)?;
        if !rsp.sw.is_ok() {
            report.fail(
                "CL extended APDU (case 4E)",
                format!("MSE repeat failed: {}", rsp.sw),
            );
            return Ok(());
        }
        let ga1_ext = Command::new(0x10, 0x86, 0x00, 0x00)
            .with_data([0x7C, 0x00])
            .with_ne(MAX_NE_EXTENDED)
            .with_encoding(Encoding::Extended);
        match t.transmit(&ga1_ext) {
            Ok(rsp) => match parse_encrypted_nonce(&rsp) {
                Ok(z) => report.pass(
                    "CL extended APDU (case 4E)",
                    format!("GA1 extended -> {}-byte nonce, SW {}", z.len(), rsp.sw),
                ),
                Err(e) => report.fail("CL extended APDU (case 4E)", e.to_string()),
            },
            Err(e) => report.fail("CL extended APDU (case 4E)", e.to_string()),
        }

        // 6. Full PACE with the CAN (M2). The CAN is only read from the
        //    environment and never stored.
        match can_from_env() {
            Some(Ok(password)) => match kye_pace::establish(&mut t, pace, &password) {
                Ok(session) => report.pass(
                    "CL PACE (CAN)",
                    format!("T_PICC verified, {:?}", session.keys),
                ),
                Err(e) => report.fail("CL PACE (CAN)", e.to_string()),
            },
            Some(Err(e)) => report.fail("CL PACE (CAN)", e.to_string()),
            None => println!("  Note: KYE_CAN not set, PACE skipped."),
        }
    }
    // Reset the card so the half-finished PACE run leaves no state behind.
    tx.end(true)?;
    card.disconnect_reset()?;
    Ok(())
}

fn can_from_env() -> Option<Result<Password, kye_pace::PaceError>> {
    let can = zeroize::Zeroizing::new(std::env::var("KYE_CAN").ok()?);
    Some(Password::can(&can))
}

fn select_pace_info<'a>(ca: &'a CardAccess, report: &mut Report) -> Result<&'a PaceInfo, BoxError> {
    let pace = ca
        .find_pace(PaceProtocol::ECDH_GM_AES_128)
        .ok_or("EF.CardAccess announces no id-PACE-ECDH-GM-AES-CBC-CMAC-128")?;
    let params = pace.domain_parameters();
    let detail = format!("{} v{}, params {:?}", pace.oid, pace.version, params);
    if params.is_none_or(|p| p == StandardizedDomainParameters::BrainpoolP256r1) {
        report.pass("PACEInfo ECDH-GM AES-128", detail);
    } else {
        report.fail(
            "PACEInfo ECDH-GM AES-128",
            format!("unexpected domain parameters: {detail}"),
        );
    }
    Ok(pace)
}

fn read_card_access<T: CardTransport>(t: &mut T) -> Result<Vec<u8>, BoxError> {
    let first = t.transmit(&read_binary_sfi(EF_CARD_ACCESS.sfi, 0, MAX_NE_SHORT))?;
    if !first.sw.is_ok() && first.sw != StatusWord::END_OF_FILE {
        return Err(format!("READ BINARY EF.CardAccess failed: {}", first.sw).into());
    }
    let mut data = first.data;
    let total = CardAccess::encoded_len(&data).ok_or("EF.CardAccess does not start with a SET")?;
    if data.len() >= total {
        data.truncate(total);
        return Ok(data);
    }
    let rest = read_offset(t, data.len(), total - data.len())?;
    data.extend_from_slice(&rest);
    Ok(data)
}

/// Reads `len` bytes from the current EF starting at `offset`.
fn read_offset<T: CardTransport>(
    t: &mut T,
    offset: usize,
    len: usize,
) -> Result<Vec<u8>, BoxError> {
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        let off = u16::try_from(offset + out.len())?;
        let ne = (len - out.len()).min(MAX_NE_SHORT);
        let rsp = t.transmit(&read_binary(off, ne))?;
        if rsp.data.is_empty() || (!rsp.sw.is_ok() && rsp.sw != StatusWord::END_OF_FILE) {
            return Err(format!("READ BINARY at offset {off} failed: {}", rsp.sw).into());
        }
        out.extend_from_slice(&rsp.data);
    }
    out.truncate(len);
    Ok(out)
}

fn expect_ok<T: CardTransport>(t: &mut T, name: &str, cmd: &Command) -> Result<(), BoxError> {
    let rsp = t.transmit(cmd)?;
    if rsp.sw.is_ok() {
        Ok(())
    } else {
        Err(format!("{name} failed: {}", rsp.sw).into())
    }
}

fn print_status_vd(data: &[u8], report: &mut Report) {
    match StatusVd::parse(data) {
        Ok(s) => {
            println!("  StatusVD: update incomplete = {}", s.update_incomplete);
            println!("            last update      = {}", s.timestamp);
            println!(
                "            version XML      = {} (raw {})",
                s.version_xml,
                Hex(&s.version_xml.0)
            );
            println!(
                "            version storage  = {} (raw {})",
                s.version_storage,
                Hex(&s.version_storage.0)
            );
        }
        Err(e) => report.fail("parse EF.StatusVD", e.to_string()),
    }
}
