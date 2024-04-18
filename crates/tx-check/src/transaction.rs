use std::collections::HashMap;

#[cfg(feature = "bulletproof")]
use bitcoin::secp256k1::Secp256k1;
use bitcoin::{self, Transaction, TxIn, TxOut};

#[cfg(feature = "bulletproof")]
use yuv_pixels::k256::elliptic_curve::group::GroupEncoding;
use yuv_pixels::{CheckableProof, Chroma, P2WPKHWintessData, PixelProof};
use yuv_types::ProofMap;
use yuv_types::{YuvTransaction, YuvTxType};

use crate::errors::CheckError;

/// Checks transactions' correctness in terms of conservation rules and provided proofs.
pub fn check_transaction(yuv_tx: &YuvTransaction) -> Result<(), CheckError> {
    match &yuv_tx.tx_type {
        YuvTxType::Issue { output_proofs } => check_issue(&yuv_tx.bitcoin_tx, output_proofs),
        YuvTxType::Transfer {
            input_proofs,
            output_proofs,
        } => check_transfer(&yuv_tx.bitcoin_tx, input_proofs, output_proofs),
        // To check transaction's correctness we need to have list of transactions that are frozen.
        // That's why we skip it on this step.
        YuvTxType::FreezeToggle { .. } => Ok(()),
    }
}

pub(crate) fn check_issue(tx: &Transaction, outputs: &ProofMap) -> Result<(), CheckError> {
    if !check_same_chroma_proofs(&outputs.values().collect::<Vec<_>>()) {
        return Err(CheckError::NotSameChroma);
    }

    let gathered_outputs = extract_from_iterable_by_proof_map(outputs, &tx.output)?;

    for ProofForCheck {
        inner,
        vout,
        statement: txout,
    } in gathered_outputs.iter()
    {
        inner
            .checked_check_by_output(txout)
            .map_err(|error| CheckError::InvalidProof {
                proof: Box::new((*inner).clone()),
                vout: *vout,
                error,
            })?;
    }

    check_issue_conservation_rules(&gathered_outputs, tx)?;

    Ok(())
}

pub(crate) fn check_transfer(
    tx: &Transaction,
    inputs: &ProofMap,
    outputs: &ProofMap,
) -> Result<(), CheckError> {
    check_tx_same_chroma(
        &inputs.values().collect::<Vec<_>>(),
        &outputs.values().collect::<Vec<_>>(),
    )?;

    let gathered_inputs = extract_from_iterable_by_proof_map(inputs, &tx.input)?;
    let gathered_outputs = extract_from_iterable_by_proof_map(outputs, &tx.output)?;

    for ProofForCheck {
        inner,
        vout,
        statement: txin,
    } in gathered_inputs.iter()
    {
        inner
            .checked_check_by_input(txin)
            .map_err(|error| CheckError::InvalidProof {
                proof: Box::new((*inner).clone()),
                vout: *vout,
                error,
            })?;
    }

    for ProofForCheck {
        inner,
        vout,
        statement: txout,
    } in gathered_outputs.iter()
    {
        inner
            .checked_check_by_output(txout)
            .map_err(|error| CheckError::InvalidProof {
                proof: Box::new((*inner).clone()),
                vout: *vout,
                error,
            })?;
    }

    #[cfg(feature = "bulletproof")]
    if is_bulletproof(inputs, outputs)? {
        are_commitments_equal(inputs, outputs)?;
        return Ok(());
    }

    check_transfer_conservation_rules(&gathered_inputs, &gathered_outputs)?;

    Ok(())
}

/// Check that all proofs for transaction has the same chroma.
fn check_tx_same_chroma(
    input_proofs: &[&PixelProof],
    output_proofs: &[&PixelProof],
) -> Result<(), CheckError> {
    let first_input = input_proofs
        .first()
        .ok_or_else(|| CheckError::EmptyInputs)?;

    let Some(first_output) = output_proofs.first() else {
        return Err(CheckError::EmptyOutputs);
    };

    if first_input.pixel().chroma != first_output.pixel().chroma {
        return Err(CheckError::NotSameChroma);
    }

    Ok(())
}

pub(crate) struct ProofForCheck<'b, T> {
    /// Statement we will validate (tx input or tx output)
    pub(crate) statement: T,
    /// Number of output in the transaction.
    pub(crate) vout: u32,
    /// Proof we are validating.
    pub(crate) inner: &'b PixelProof,
}

impl<'a, T> ProofForCheck<'a, T> {
    pub(crate) fn new(statement: T, vout: u32, proof: &'a PixelProof) -> Self {
        Self {
            statement,
            vout,
            inner: proof,
        }
    }
}

/// Generic function for extracting proofs with related to them inputs or
/// outputs.
pub(crate) fn extract_from_iterable_by_proof_map<'a, T>(
    proof_map: &'a ProofMap,
    iterable: &'a [T],
) -> Result<Vec<ProofForCheck<'a, &'a T>>, CheckError> {
    let mut gathered_proofs = Vec::new();

    for (vout, proof) in proof_map {
        let item = iterable
            .get(*vout as usize)
            .ok_or(CheckError::ProofMappedToNotExistingInputOutput)?;

        let proof_for_check = ProofForCheck::new(item, *vout, proof);

        gathered_proofs.push(proof_for_check);
    }

    Ok(gathered_proofs)
}

/// Check that proofs of the transaction do not violate conservation rules. For transfer
/// check that the sum of inputs equals the sum of the outputs.
pub(crate) fn check_transfer_conservation_rules(
    inputs: &[ProofForCheck<&TxIn>],
    outputs: &[ProofForCheck<&TxOut>],
) -> Result<(), CheckError> {
    let input_chromas = sum_amount_by_chroma(inputs);
    let output_chromas: HashMap<Chroma, u64> = sum_amount_by_chroma(outputs);

    if input_chromas != output_chromas {
        return Err(CheckError::ConservationRulesViolated);
    }

    Ok(())
}

fn sum_amount_by_chroma<T>(proofs: &[ProofForCheck<T>]) -> HashMap<Chroma, u64> {
    let mut chromas: HashMap<Chroma, u64> = HashMap::new();

    for proof in proofs {
        let pixel = proof.inner.pixel();

        let chroma_sum = chromas.entry(pixel.chroma).or_insert(0);
        *chroma_sum += pixel.luma.amount;
    }

    chromas
}

/// Check that proofs of the issuance do not violate conservation rules (that chroma (asset type) equals to issuer public key)
pub(crate) fn check_issue_conservation_rules(
    outputs: &[ProofForCheck<&TxOut>],
    tx: &Transaction,
) -> Result<(), CheckError> {
    // Find transaction input which has public key equal to chroma of output.
    //
    // NOTE: we assume that transaction has only one type of chroma.
    let Some(first_output) = outputs.first() else {
        return Err(CheckError::EmptyOutputs);
    };

    let input = find_issuer_in_txinputs(&tx.input, &first_output.inner.pixel().chroma);

    // If there is no input with chroma of output, then issuer is not the owner of the chroma.
    if input.is_none() {
        return Err(CheckError::IssuerNotOwner);
    }

    Ok(())
}

/// Check that all proofs has the same chroma, assuming that all proofs are valid.
///
/// It is needed only for single-chroma transactions, in future when multi-chroma transactions will be
/// supported, it can be removed.
fn check_same_chroma_proofs(proofs: &[&PixelProof]) -> bool {
    proofs.windows(2).all(|w| {
        let [previous, next] = w else {
            panic!("Windows method is set to 2");
        };

        previous.pixel().chroma == next.pixel().chroma
    })
}

/// Find issuer of the transaction in the inputs by chroma.
pub(crate) fn find_issuer_in_txinputs<'a>(inputs: &'a [TxIn], chroma: &Chroma) -> Option<&'a TxIn> {
    inputs.iter().find(|input| {
        // Skip entry if it's not p2wpkh
        //
        // TODO: may be, in future, we should support other types of inputs.
        let Ok(witness) = P2WPKHWintessData::from_witness(&input.witness) else {
            return false;
        };

        let (xonly_public_key, _parity) = witness.pubkey.inner.x_only_public_key();

        &xonly_public_key == chroma.xonly()
    })
}

#[cfg(feature = "bulletproof")]
fn is_bulletproof(inputs: &ProofMap, outputs: &ProofMap) -> Result<bool, CheckError> {
    let mut was_found = false;
    for proof in inputs.values().chain(outputs.values()) {
        let is_bulletproof = proof.is_bulletproof();

        if was_found && !is_bulletproof {
            return Err(CheckError::MixedBulletproofsAndNonBulletproofs);
        }

        if is_bulletproof {
            was_found = true;
        }
    }

    Ok(was_found)
}

#[cfg(feature = "bulletproof")]
fn are_commitments_equal(
    inputs_proofs: &ProofMap,
    outputs_proofs: &ProofMap,
) -> Result<bool, CheckError> {
    let (owners, commits): (Vec<_>, Vec<_>) = inputs_proofs
        .iter()
        .chain(outputs_proofs.iter())
        .map(|(_, proof)| {
            let proof = proof
                .get_bulletproof()
                .expect("Bulletproofs should be checked");

            (proof.commiter, proof.commitment)
        })
        .unzip();

    let merged_owner = owners
        .into_iter()
        .reduce(|acc, owner| {
            acc.combine(&owner.negate(&Secp256k1::new()))
                .expect("Owners should be valid")
        })
        .ok_or(CheckError::AtLeastOneCommitment)?;

    let raw_merged_commit = commits
        .into_iter()
        .reduce(|acc, commit| acc - commit)
        .ok_or(CheckError::AtLeastOneCommitment)?;

    Ok(merged_owner.serialize() == raw_merged_commit.to_bytes().as_slice())
}
