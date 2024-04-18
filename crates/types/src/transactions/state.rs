/// Transaction states that are stored in storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[repr(u8)]
pub enum TxState {
    /// Transaction is pending to be checked.
    Pending = 1,

    /// Transaction is checked and ready to be attached.
    Checked = 2,
}
