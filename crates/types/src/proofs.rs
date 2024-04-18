use alloc::collections::BTreeMap;
use yuv_pixels::PixelProof;

/// Contains proofs for inputs or outputs of the YUV Transaction.
///
/// Maps inputs or outputs ids to [`PixelProof`]s.
pub type ProofMap = BTreeMap<u32, PixelProof>;

/// Contains proofs for inputs and outputs of the YUV Transaction.
#[derive(Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Proofs {
    #[cfg_attr(feature = "serde", serde(default))]
    pub input: ProofMap,
    pub output: ProofMap,
}
