//! Typed view on EF.PD and EF.VD (gemSpec_eGK_Fach_VSDM, schema 5.x).
//!
//! Fields are looked up by local element name, so schema namespaces and minor
//! nesting differences between versions do not matter. The full element tree
//! is kept for display of fields this model does not know.

use crate::EgkError;
use crate::status_vd::StatusVd;
use crate::vsd::{RawVsd, VdOffsets};
use crate::xml::{Element, parse_gzip_xml};

/// Street address (`StrassenAdresse`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StreetAddress {
    pub street: Option<String>,
    pub house_number: Option<String>,
    pub addition: Option<String>,
    pub postal_code: Option<String>,
    pub city: Option<String>,
    pub country: Option<String>,
}

/// Postbox address (`PostfachAdresse`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PostboxAddress {
    pub postbox: Option<String>,
    pub postal_code: Option<String>,
    pub city: Option<String>,
    pub country: Option<String>,
}

/// Personal data of the insured person (EF.PD).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonalData {
    pub schema_version: Option<String>,
    pub insurant_id: Option<String>,
    /// `YYYYMMDD` as stored on the card.
    pub birth_date: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub sex: Option<String>,
    pub name_prefix: Option<String>,
    pub name_addition: Option<String>,
    pub title: Option<String>,
    pub street_address: Option<StreetAddress>,
    pub postbox_address: Option<PostboxAddress>,
    pub xml: Element,
}

/// General insurance data (EF.VD).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsuranceData {
    pub schema_version: Option<String>,
    /// `YYYYMMDD`.
    pub coverage_start: Option<String>,
    /// `YYYYMMDD`, absent for open-ended coverage.
    pub coverage_end: Option<String>,
    pub payer_id: Option<String>,
    pub payer_name: Option<String>,
    pub payer_country: Option<String>,
    pub billing_payer_id: Option<String>,
    pub billing_payer_name: Option<String>,
    /// `Versichertenart`: 1 member, 3 family member, 5 pensioner.
    pub insurant_type: Option<String>,
    pub special_group: Option<String>,
    pub dmp: Option<String>,
    /// `WOP` (Wohnortprinzip) code.
    pub wop: Option<String>,
    pub xml: Element,
}

/// Complete, parsed VSD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vsd {
    pub status: StatusVd,
    pub personal: PersonalData,
    pub insurance: InsuranceData,
}

impl Vsd {
    pub fn parse(raw: &RawVsd) -> Result<Self, EgkError> {
        Ok(Self {
            status: StatusVd::parse(&raw.status_vd)?,
            personal: PersonalData::parse_ef(&raw.pd)?,
            insurance: InsuranceData::parse_ef(&raw.vd)?,
        })
    }
}

fn schema_version(root: &Element) -> Option<String> {
    root.attribute("CDM_VERSION").map(str::to_owned)
}

impl PersonalData {
    /// Parses EF.PD: `len(2, BE) || gzip(XML)`.
    pub fn parse_ef(ef: &[u8]) -> Result<Self, EgkError> {
        let len = crate::vsd::pd_total_len(ef)?;
        let gz = ef.get(2..len).ok_or(EgkError::Format {
            file: "EF.PD",
            reason: "shorter than announced",
        })?;
        Ok(Self::from_xml(parse_gzip_xml(gz, "EF.PD")?))
    }

    pub fn from_xml(xml: Element) -> Self {
        let t = |path: &[&str]| xml.find_text(path);
        let street_address = xml.find(&["StrassenAdresse"]).map(|a| StreetAddress {
            street: a.find_text(&["Strasse"]),
            house_number: a.find_text(&["Hausnummer"]),
            addition: a.find_text(&["Anschriftenzusatz"]),
            postal_code: a.find_text(&["Postleitzahl"]),
            city: a.find_text(&["Ort"]),
            country: a.find_text(&["Wohnsitzlaendercode"]),
        });
        let postbox_address = xml.find(&["PostfachAdresse"]).map(|a| PostboxAddress {
            postbox: a.find_text(&["Postfach"]),
            postal_code: a.find_text(&["Postleitzahl"]),
            city: a.find_text(&["Ort"]),
            country: a.find_text(&["Wohnsitzlaendercode"]),
        });
        Self {
            schema_version: schema_version(&xml),
            insurant_id: t(&["Versicherten_ID"]),
            birth_date: t(&["Person", "Geburtsdatum"]),
            first_name: t(&["Person", "Vorname"]),
            last_name: t(&["Person", "Nachname"]),
            sex: t(&["Person", "Geschlecht"]),
            name_prefix: t(&["Person", "Vorsatzwort"]),
            name_addition: t(&["Person", "Namenszusatz"]),
            title: t(&["Person", "Titel"]),
            street_address,
            postbox_address,
            xml,
        }
    }
}

impl InsuranceData {
    /// Parses EF.VD: offset header, then gzip(XML) from `offStartVD` to
    /// `offEndVD` (inclusive). The GVD copy is ignored.
    pub fn parse_ef(ef: &[u8]) -> Result<Self, EgkError> {
        let off = VdOffsets::parse(ef)?;
        let gz = ef
            .get(usize::from(off.start_vd)..off.total_len())
            .ok_or(EgkError::Format {
                file: "EF.VD",
                reason: "shorter than announced",
            })?;
        Ok(Self::from_xml(parse_gzip_xml(gz, "EF.VD")?))
    }

    pub fn from_xml(xml: Element) -> Self {
        let t = |path: &[&str]| xml.find_text(path);
        Self {
            schema_version: schema_version(&xml),
            coverage_start: t(&["Versicherungsschutz", "Beginn"]),
            coverage_end: t(&["Versicherungsschutz", "Ende"]),
            payer_id: t(&[
                "Versicherungsschutz",
                "Kostentraeger",
                "Kostentraegerkennung",
            ]),
            payer_name: t(&["Versicherungsschutz", "Kostentraeger", "Name"]),
            payer_country: t(&[
                "Versicherungsschutz",
                "Kostentraeger",
                "Kostentraegerlaendercode",
            ]),
            billing_payer_id: t(&["AbrechnenderKostentraeger", "Kostentraegerkennung"]),
            billing_payer_name: t(&["AbrechnenderKostentraeger", "Name"]),
            insurant_type: t(&["Versichertenart"]),
            special_group: t(&["Besondere_Personengruppe"]),
            dmp: t(&["DMP_Kennzeichnung"]),
            wop: t(&["WOP"]),
            xml,
        }
    }
}

/// Formats `YYYYMMDD` as `DD.MM.YYYY`; other input is returned unchanged.
pub fn format_date(d: &str) -> String {
    if d.len() == 8 && d.bytes().all(|b| b.is_ascii_digit()) {
        format!("{}.{}.{}", &d[6..8], &d[4..6], &d[0..4])
    } else {
        d.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    // Synthetic documents in the shape of the VSDM 5.2 schema. All values are made up.
    const PD_XML: &str = r#"<?xml version="1.0" encoding="ISO-8859-15" standalone="yes"?>
<vsda:UC_PersoenlicheVersichertendatenXML xmlns:vsda="http://ws.gematik.de/fa/vsdm/vsd/v5.2" CDM_VERSION="5.2.0">
  <vsda:Versicherter>
    <vsda:Versicherten_ID>X110000000</vsda:Versicherten_ID>
    <vsda:Person>
      <vsda:Geburtsdatum>19800229</vsda:Geburtsdatum>
      <vsda:Vorname>Jürgen</vsda:Vorname>
      <vsda:Nachname>Testmann-Größe</vsda:Nachname>
      <vsda:Geschlecht>M</vsda:Geschlecht>
      <vsda:Titel>Dr.</vsda:Titel>
      <vsda:StrassenAdresse>
        <vsda:Postleitzahl>10117</vsda:Postleitzahl>
        <vsda:Ort>Berlin</vsda:Ort>
        <vsda:Land><vsda:Wohnsitzlaendercode>D</vsda:Wohnsitzlaendercode></vsda:Land>
        <vsda:Strasse>Musterstraße</vsda:Strasse>
        <vsda:Hausnummer>1 €</vsda:Hausnummer>
      </vsda:StrassenAdresse>
    </vsda:Person>
  </vsda:Versicherter>
</vsda:UC_PersoenlicheVersichertendatenXML>"#;

    const VD_XML: &str = r#"<?xml version="1.0" encoding="ISO-8859-15" standalone="yes"?>
<UC_AllgemeineVersicherungsdatenXML xmlns="http://ws.gematik.de/fa/vsdm/vsd/v5.2" CDM_VERSION="5.2.0">
  <Versicherter>
    <Versicherungsschutz>
      <Beginn>20200101</Beginn>
      <Kostentraeger>
        <Kostentraegerkennung>100000000</Kostentraegerkennung>
        <Kostentraegerlaendercode>D</Kostentraegerlaendercode>
        <Name>Test-Krankenkasse</Name>
        <AbrechnenderKostentraeger>
          <Kostentraegerkennung>100000001</Kostentraegerkennung>
          <Kostentraegerlaendercode>D</Kostentraegerlaendercode>
          <Name>Abrechnungsstelle</Name>
        </AbrechnenderKostentraeger>
      </Kostentraeger>
    </Versicherungsschutz>
    <Zusatzinfos>
      <ZusatzinfosGKV>
        <Versichertenart>1</Versichertenart>
        <Zusatzinfos_Abrechnung_GKV>
          <WOP>72</WOP>
        </Zusatzinfos_Abrechnung_GKV>
      </ZusatzinfosGKV>
    </Zusatzinfos>
  </Versicherter>
</UC_AllgemeineVersicherungsdatenXML>"#;

    fn gzip_latin9(xml: &str) -> Vec<u8> {
        let (bytes, _, unmappable) = encoding_rs::ISO_8859_15.encode(xml);
        assert!(!unmappable);
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(&bytes).unwrap();
        enc.finish().unwrap()
    }

    fn ef_pd() -> Vec<u8> {
        let gz = gzip_latin9(PD_XML);
        let mut ef = (gz.len() as u16).to_be_bytes().to_vec();
        ef.extend(gz);
        ef
    }

    fn ef_vd() -> Vec<u8> {
        let gz = gzip_latin9(VD_XML);
        let end = (8 + gz.len() - 1) as u16;
        let mut ef = vec![0x00, 0x08];
        ef.extend(end.to_be_bytes());
        ef.extend([0xFF, 0xFF, 0xFF, 0xFF]);
        ef.extend(gz);
        ef
    }

    fn ef_status() -> Vec<u8> {
        let mut v = b"120240131120000".to_vec();
        v.extend([0x00, 0x50, 0x02, 0x00, 0x00, 0x00, 0x30, 0x00, 0x00, 0x04]);
        v
    }

    #[test]
    fn parse_personal_data() {
        let pd = PersonalData::parse_ef(&ef_pd()).unwrap();
        assert_eq!(pd.schema_version.as_deref(), Some("5.2.0"));
        assert_eq!(pd.insurant_id.as_deref(), Some("X110000000"));
        assert_eq!(pd.first_name.as_deref(), Some("Jürgen"));
        assert_eq!(pd.last_name.as_deref(), Some("Testmann-Größe"));
        assert_eq!(pd.title.as_deref(), Some("Dr."));
        assert_eq!(pd.name_prefix, None);
        assert_eq!(format_date(pd.birth_date.as_deref().unwrap()), "29.02.1980");
        let a = pd.street_address.unwrap();
        assert_eq!(a.street.as_deref(), Some("Musterstraße"));
        assert_eq!(a.house_number.as_deref(), Some("1 €"));
        assert_eq!(a.country.as_deref(), Some("D"));
        assert!(pd.postbox_address.is_none());
    }

    #[test]
    fn parse_insurance_data() {
        let vd = InsuranceData::parse_ef(&ef_vd()).unwrap();
        assert_eq!(vd.coverage_start.as_deref(), Some("20200101"));
        assert_eq!(vd.coverage_end, None);
        assert_eq!(vd.payer_id.as_deref(), Some("100000000"));
        assert_eq!(vd.payer_name.as_deref(), Some("Test-Krankenkasse"));
        assert_eq!(vd.billing_payer_id.as_deref(), Some("100000001"));
        assert_eq!(vd.billing_payer_name.as_deref(), Some("Abrechnungsstelle"));
        assert_eq!(vd.insurant_type.as_deref(), Some("1"));
        assert_eq!(vd.wop.as_deref(), Some("72"));
    }

    #[test]
    fn parse_complete_vsd() {
        let raw = RawVsd {
            status_vd: ef_status(),
            pd: ef_pd(),
            vd: ef_vd(),
        };
        let vsd = Vsd::parse(&raw).unwrap();
        assert!(vsd.status.update_incomplete);
        assert_eq!(vsd.personal.first_name.as_deref(), Some("Jürgen"));
    }

    #[test]
    fn truncated_files() {
        let pd = ef_pd();
        assert!(PersonalData::parse_ef(&pd[..pd.len() - 1]).is_err());
        let vd = ef_vd();
        assert!(InsuranceData::parse_ef(&vd[..vd.len() - 1]).is_err());
        let mut corrupt = ef_pd();
        corrupt[5] ^= 0xFF;
        assert!(PersonalData::parse_ef(&corrupt).is_err());
    }
}
