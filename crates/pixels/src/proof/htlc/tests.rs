#[cfg(feature = "consensus")]
use alloc::vec::Vec;
use core::str::FromStr;

#[cfg(feature = "consensus")]
use bitcoin::consensus::{Decodable, Encodable};
use bitcoin::{
    hashes::{hash160, hex::FromHex},
    secp256k1::PublicKey,
    Script, TxIn, TxOut, Witness, XOnlyPublicKey,
};
use once_cell::sync::Lazy;

use crate::Chroma;

use super::*;

static OFFERED_HTLC_SCRIPT_1: Lazy<Script> = Lazy::new(|| {
    "76a91414011f7254d96b819c76986c277d115efce6f7b58763ac67210394854aa6e\
        ab5b2a8122cc726e9dded053a2184d88256816826d6231c068d4a5b7c8201208\
        76475527c21030d417a46946384f88d5f3337267c5e579765875dc4daca813e2\
        1734b140639e752ae67a914b43e1b38138a41b37f7cd9a1d274bc63e3a9b5d18\
        8ac6868"
        .parse()
        .unwrap()
});

static RECEIVED_HTLC_SCRIPT_1: Lazy<Script> = Lazy::new(|| {
    "76a91414011f7254d96b819c76986c277d115efce6f7b58763ac67210394854aa6e\
        ab5b2a8122cc726e9dded053a2184d88256816826d6231c068d4a5b7c8201208\
        763a914b8bcb07f6344b42ab04250c86a6e8b75d3fdbbc688527c21030d417a4\
        6946384f88d5f3337267c5e579765875dc4daca813e21734b140639e752ae677\
        502f401b175ac6868"
        .parse()
        .unwrap()
});

static REVOCATION_PUBKEY_HASH: Lazy<hash160::Hash> =
    Lazy::new(|| hash160::Hash::from_hex("14011f7254d96b819c76986c277d115efce6f7b5").unwrap());

static REMOTE_HTLC_PUBKEY: Lazy<PublicKey> = Lazy::new(|| {
    "0394854aa6eab5b2a8122cc726e9dded053a2184d88256816826d6231c068d4a5b"
        .parse()
        .unwrap()
});

static LOCAL_HTLC_PUBKEY: Lazy<PublicKey> = Lazy::new(|| {
    "030d417a46946384f88d5f3337267c5e579765875dc4daca813e21734b140639e7"
        .parse()
        .unwrap()
});

static OFFERED_PAYMENT_HASH: Lazy<hash160::Hash> =
    Lazy::new(|| hash160::Hash::from_hex("b43e1b38138a41b37f7cd9a1d274bc63e3a9b5d1").unwrap());

static RECEIVED_PAYMENT_HASH: Lazy<hash160::Hash> =
    Lazy::new(|| hash160::Hash::from_hex("b8bcb07f6344b42ab04250c86a6e8b75d3fdbbc6").unwrap());

#[test]
fn test_bolt3_appendix_c_htlc_scripts() {
    let htlc_script = LightningHtlcData::offered(
        *REVOCATION_PUBKEY_HASH,
        *REMOTE_HTLC_PUBKEY,
        *LOCAL_HTLC_PUBKEY,
        *OFFERED_PAYMENT_HASH,
    );

    assert_eq!(Script::from(htlc_script), *OFFERED_HTLC_SCRIPT_1);

    let cltv_expiry = 500;
    let htlc_script = LightningHtlcData::received(
        *REVOCATION_PUBKEY_HASH,
        *REMOTE_HTLC_PUBKEY,
        *LOCAL_HTLC_PUBKEY,
        *RECEIVED_PAYMENT_HASH,
        cltv_expiry,
    );

    assert_eq!(Script::from(htlc_script), *RECEIVED_HTLC_SCRIPT_1);
}

static CHROMA: Lazy<Chroma> = Lazy::new(|| {
    XOnlyPublicKey::from_str("0677b5829356bb5e0c0808478ac150a500ceab4894d09854b0f75fbe7b4162f8")
        .expect("Should be valid chroma")
        .into()
});

#[test]
fn test_proof_simple_checks() {
    let data = LightningHtlcData::offered(
        *REVOCATION_PUBKEY_HASH,
        *REMOTE_HTLC_PUBKEY,
        *LOCAL_HTLC_PUBKEY,
        *OFFERED_PAYMENT_HASH,
    );

    let pixel = Pixel::new(100, *CHROMA);

    let proof = LightningHtlcProof::new(pixel, data);

    let script = Script::from(LightningHtlcScript::from(&proof));

    let script_pubkey = script.to_v0_p2wsh();

    let txout = TxOut {
        script_pubkey,
        value: 100,
    };

    let got = proof.checked_check_by_output(&txout);

    assert!(got.is_ok(), "Check by output failed, got: {:?}", got);

    // Insert into witness script as bytes:
    let witness = Witness::from_vec(alloc::vec![script.as_bytes().to_vec()]);

    let txin = TxIn {
        witness,
        ..Default::default()
    };

    let got = proof.checked_check_by_input(&txin);

    assert!(got.is_ok(), "Check by input failed, got: {:?}", got);
}

#[test]
#[cfg(feature = "consensus")]
fn test_lightning_htlc_data_consensus_encode() {
    let data = LightningHtlcData::new(
        *REVOCATION_PUBKEY_HASH,
        *REMOTE_HTLC_PUBKEY,
        *LOCAL_HTLC_PUBKEY,
        *OFFERED_PAYMENT_HASH,
        HtlcScriptKind::Received { cltv_expiry: 100 },
    );

    let mut bytes = Vec::new();

    data.consensus_encode(&mut bytes)
        .expect("failed to encode data");

    let decoded_data =
        LightningHtlcData::consensus_decode(&mut bytes.as_slice()).expect("failed to decode data");

    assert_eq!(data, decoded_data, "Converting back and forth should work");
}
