use bitcoin::hashes::{sha256::Hash as Sha256Hash, Hash, HashEngine};
use core::ops::Deref;

use crate::Pixel;

/// A hash of the YUV pixel data that uniquely identifies a pixel (coin).
///
/// Defined as: `PXH = hash(hash(Y) || UV)`, where `Y` - is luma (amount),
/// and `UV` - is token type (issuer public key).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PixelHash(pub Sha256Hash);

impl Deref for PixelHash {
    type Target = Sha256Hash;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Pixel> for PixelHash {
    fn from(pixel: Pixel) -> Self {
        Self::from(&pixel)
    }
}

impl From<Sha256Hash> for PixelHash {
    fn from(hash: Sha256Hash) -> Self {
        Self(hash)
    }
}

impl From<&Pixel> for PixelHash {
    fn from(pixel: &Pixel) -> Self {
        let mut hash_engine = Sha256Hash::engine();

        hash_engine.input(&pixel.luma.to_bytes());
        // hash(Y)
        let amount_hashed = Sha256Hash::from_engine(hash_engine);

        let mut hash_engine = Sha256Hash::engine();
        // hash(hash(Y) || UV)
        hash_engine.input(amount_hashed.as_inner());

        // Skip first byte of the public key (0x02 or 0x03) and hash the rest.
        hash_engine.input(&pixel.chroma.xonly().serialize());

        let pxh = Sha256Hash::from_engine(hash_engine);

        Self(pxh)
    }
}

#[cfg(test)]
mod tests {
    use crate::pixel::BLINDING_FACTOR_SIZE;
    use crate::{Luma, Pixel, PixelHash};
    use bitcoin::hashes::sha256::Hash;
    use bitcoin::hashes::Hash as BitcoinHash;
    use bitcoin::util::key::PublicKey;
    use core::str::FromStr;
    use once_cell::sync::Lazy;

    const AMOUNT: u64 = 100;

    static PUBKEY: Lazy<PublicKey> = Lazy::new(|| {
        PublicKey::from_str("03ab5575d69e46968a528cd6fa2a35dd7808fea24a12b41dc65c7502108c75f9a9")
            .unwrap()
    });

    static MOCKED_HASH_STR: Lazy<Hash> = Lazy::new(|| {
        Hash::from_slice(
            &hex::decode("8a5cc8c0c3fabb1bd5ba9f961b6b432c2de5e1272a7b6c98b2aaabc7c5e41c8e")
                .unwrap(),
        )
        .unwrap()
    });

    #[test]
    fn test_hash() {
        let pixel = Pixel::new(Luma::new(AMOUNT, [0; BLINDING_FACTOR_SIZE]), *PUBKEY);

        assert_eq!(PixelHash::from(&pixel).0, *MOCKED_HASH_STR);
    }
}
