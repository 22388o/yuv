use bitcoin::{Network, PrivateKey, PublicKey};
use clap::Subcommand;
use color_eyre::eyre;

use yuv_pixels::k256::{self, elliptic_curve::sec1::FromEncodedPoint};

use crate::context::Context;

use self::{check::CheckArgs, issue::IssueArgs, transfer::TransferArgs};

mod check;
mod dh;
mod issue;
mod transfer;

const HKDF_SALT: &[u8] = b"43f905cb425b135f2ec3671bffd6643b8b8239fc8db5c529339f41c7d29bff5a";
const HKDF_INFO: &[u8] = b"ecdh key agreement";

#[derive(Subcommand, Debug)]
pub enum BulletproofCommands {
    // bulletproof issue
    Issue(IssueArgs),
    // bulletproof transfer
    Transfer(TransferArgs),
    // bulletproof check
    Check(CheckArgs),
    // bulletproof dh
    Dh(dh::DhArgs),
}

pub async fn run(cmd: BulletproofCommands, context: Context) -> eyre::Result<()> {
    match cmd {
        BulletproofCommands::Issue(args) => issue::run(args, context).await,
        BulletproofCommands::Transfer(args) => transfer::run(args, context).await,
        BulletproofCommands::Check(args) => check::run(args, context).await,
        BulletproofCommands::Dh(args) => dh::run(args, context),
    }
}

pub fn sha256(data: &[u8]) -> eyre::Result<[u8; 32]> {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();

    hasher.update(data);

    Ok(hasher.finalize().into())
}

pub fn ecdh(key: PrivateKey, pub_key: PublicKey, network: Network) -> eyre::Result<PrivateKey> {
    let key = k256::SecretKey::from_slice(&key.to_bytes())?;

    let encoded_pub_key = k256::EncodedPoint::from_bytes(pub_key.to_bytes())?;
    let pub_key =
        k256::PublicKey::from_encoded_point(&encoded_pub_key).expect("failed to create public key");

    let result_key = _ecdh(key, pub_key)?;

    Ok(PrivateKey::from_slice(&result_key.to_bytes(), network)?)
}

fn _ecdh(key: k256::SecretKey, pub_key: k256::PublicKey) -> eyre::Result<k256::SecretKey> {
    let scalar = key.to_nonzero_scalar();

    let shared_secret = k256::ecdh::diffie_hellman(&scalar, pub_key.as_affine());

    let hkdf = shared_secret.extract::<sha2::Sha256>(Some(HKDF_SALT));

    let mut data = [0u8; 32];

    hkdf.expand(HKDF_INFO, &mut data)
        .expect("failed to expand hkdf");

    Ok(k256::SecretKey::from_slice(&data)?)
}
