//! File identifiers, short file identifiers and AIDs.

/// Application identifier of DF.HCA (health care application).
pub const AID_HCA: [u8; 6] = [0xD2, 0x76, 0x00, 0x00, 0x01, 0x02];

/// An elementary file with transparent structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ef {
    pub name: &'static str,
    pub fid: u16,
    pub sfi: u8,
}

/// EF.CardAccess in the MF.
pub const EF_CARD_ACCESS: Ef = Ef {
    name: "EF.CardAccess",
    fid: 0x011C,
    sfi: 0x1C,
};

/// EF.StatusVD in DF.HCA.
pub const EF_STATUS_VD: Ef = Ef {
    name: "EF.StatusVD",
    fid: 0xD00C,
    sfi: 0x0C,
};

/// EF.PD (personal data) in DF.HCA.
pub const EF_PD: Ef = Ef {
    name: "EF.PD",
    fid: 0xD001,
    sfi: 0x01,
};

/// EF.VD (insurance data) in DF.HCA.
pub const EF_VD: Ef = Ef {
    name: "EF.VD",
    fid: 0xD002,
    sfi: 0x02,
};
