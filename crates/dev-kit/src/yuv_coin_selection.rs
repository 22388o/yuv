use crate::types::{FeeRate, Utxo, WeightedUtxo};
use bdk::Error;
use bitcoin::Script;
use yuv_pixels::Chroma;

/// Default coin selection algorithm used by transaction buileder if not
/// overridden
pub type DefaultCoinSelectionAlgorithm = YuvLargestFirstCoinSelection;

// Base weight of a Txin, not counting the weight needed for satisfying it.
// prev_txid (32 bytes) + prev_vout (4 bytes) + sequence (4 bytes)
pub(crate) const TXIN_BASE_WEIGHT: usize = (32 + 4 + 4) * 4;

#[derive(Debug)]
/// Remaining amount after performing coin selection
pub enum Excess {
    /// It's not possible to create spendable output from excess using the current drain output
    NoChange {
        /// Threshold to consider amount as dust for this particular change script_pubkey
        dust_threshold: u64,
        /// Exceeding amount of current selection over outgoing value and fee costs
        remaining_amount: u64,
        /// The calculated fee for the drain TxOut with the selected script_pubkey
        change_fee: u64,
    },
    /// It's possible to create spendable output from excess using the current drain output
    Change {
        /// Effective amount available to create change after deducting the change output fee
        amount: u64,
        /// The deducted change output fee
        fee: u64,
    },
}

/// Result of a successful coin selection
#[derive(Debug)]
pub struct YUVCoinSelectionResult {
    /// List of outputs selected for use as inputs
    pub selected: Vec<Utxo>,
    /// Total fee amount for the selected utxos in satoshis
    pub fee_amount: u64,
    /// Remaining amount after deducting fees and outgoing outputs
    pub amount: u64,
}

impl YUVCoinSelectionResult {
    /// The total value of the inputs selected.
    pub fn selected_amount(&self) -> u128 {
        self.selected
            .iter()
            .map(|u| u.yuv_txout().pixel.luma.amount as u128)
            .sum()
    }

    /// The total value of the inputs selected from the local wallet.
    pub fn local_selected_amount(&self) -> u128 {
        self.selected
            .iter()
            .map(|u| u.yuv_txout().pixel.luma.amount as u128)
            .sum()
    }
}

/// Trait for generalized coin selection algorithms
///
/// This trait can be implemented to make the [`Wallet`](crate::wallet::Wallet) use a customized coin
/// selection algorithm when it creates transactions.
pub trait YUVCoinSelectionAlgorithm: core::fmt::Debug {
    /// Perform the coin selection
    ///
    /// - `database`: a reference to the wallet's database that can be used to lookup additional
    ///               details for a specific UTXO
    /// - `required_utxos`: the utxos that must be spent regardless of `target_amount` with their
    ///                     weight cost
    /// - `optional_utxos`: the remaining available utxos to satisfy `target_amount` with their
    ///                     weight cost
    /// - `fee_rate`: fee rate to use
    /// - `target_amount`: the outgoing amount in satoshis and the fees already
    ///                    accumulated from added outputs and transaction’s header.
    /// - `drain_script`: the script to use in case of change
    fn coin_select(
        &self,
        required_utxos: Vec<WeightedUtxo>,
        optional_utxos: Vec<WeightedUtxo>,
        fee_rate: FeeRate,
        target_amount: u64,
        drain_script: &Script,
        target_token: Chroma,
    ) -> Result<YUVCoinSelectionResult, Error>;
}

/// Simple and dumb coin selection
///
/// This coin selection algorithm sorts the available UTXOs by value and then picks them starting
/// from the largest ones until the required amount is reached.
/// Simple and dumb coin selection
///
/// This coin selection algorithm sorts the available UTXOs by value and then picks them starting
/// from the largest ones until the required amount is reached.
#[derive(Debug, Default, Clone, Copy)]
pub struct YuvLargestFirstCoinSelection;

impl YUVCoinSelectionAlgorithm for YuvLargestFirstCoinSelection {
    fn coin_select(
        &self,
        required_utxos: Vec<WeightedUtxo>,
        mut optional_utxos: Vec<WeightedUtxo>,
        fee_rate: FeeRate,
        target_amount: u64,
        drain_script: &Script,
        target_chroma: Chroma,
    ) -> Result<YUVCoinSelectionResult, Error> {
        tracing::debug!(
            "target_amount = `{}`, fee_rate = `{:?}`",
            target_amount,
            fee_rate
        );

        // Filter UTXOs based on the target token.
        optional_utxos.retain(|wu| wu.utxo.yuv_txout().pixel.chroma == target_chroma);

        // We put the "required UTXOs" first and make sure the optional UTXOs are sorted,
        // initially smallest to largest, before being reversed with `.rev()`.
        let utxos = {
            optional_utxos.sort_unstable_by_key(|wu| wu.utxo.yuv_txout().pixel.luma.amount); // Sorting by amount now
            required_utxos
                .into_iter()
                .map(|utxo| (true, utxo))
                .chain(optional_utxos.into_iter().rev().map(|utxo| (false, utxo)))
        };

        select_sorted_utxos(utxos, fee_rate, target_amount, drain_script)
    }
}

/// OldestFirstCoinSelection always picks the utxo with the smallest blockheight to add to the selected coins next
///
/// This coin selection algorithm sorts the available UTXOs by blockheight and then picks them starting
/// from the oldest ones until the required amount is reached.
#[derive(Debug, Default, Clone, Copy)]
pub struct YUVOldestFirstCoinSelection;

impl YUVCoinSelectionAlgorithm for YUVOldestFirstCoinSelection {
    fn coin_select(
        &self,
        required_utxos: Vec<WeightedUtxo>,
        mut optional_utxos: Vec<WeightedUtxo>,
        fee_rate: FeeRate,
        target_amount: u64,
        drain_script: &Script,
        target_chroma: Chroma,
    ) -> Result<YUVCoinSelectionResult, Error> {
        // We put the "required UTXOs" first and make sure the optional UTXOs are sorted from
        // oldest to newest according to blocktime
        // For utxo that doesn't exist in DB, they will have lowest priority to be selected
        let utxos = {
            optional_utxos.retain(|wu| wu.utxo.yuv_txout().pixel.chroma == target_chroma);

            required_utxos
                .into_iter()
                .map(|utxo| (true, utxo))
                .chain(optional_utxos.into_iter().map(|utxo| (false, utxo)))
        };

        select_sorted_utxos(utxos, fee_rate, target_amount, drain_script)
    }
}

fn select_sorted_utxos(
    utxos: impl Iterator<Item = (bool, WeightedUtxo)>,
    fee_rate: FeeRate,
    target_amount: u64,
    _drain_script: &Script,
) -> Result<YUVCoinSelectionResult, Error> {
    let mut yuv_amount = 0;
    let mut fee_amount = 0;
    let mut satoshi_amount = 0; // Add a new variable to track the sum of txout().value
    let selected = utxos
        .scan(
            (&mut yuv_amount, &mut fee_amount, &mut satoshi_amount), // Include satoshi_amount here
            |(yuv_amount, fee_amount, satoshi_amount), (must_use, weighted_utxo)| {
                if must_use || **yuv_amount < target_amount || **satoshi_amount <= **fee_amount {
                    **fee_amount +=
                        fee_rate.fee_wu(TXIN_BASE_WEIGHT + weighted_utxo.satisfaction_weight);
                    **yuv_amount += weighted_utxo.utxo.yuv_txout().pixel.luma.amount; // Use yuv amount here
                    **satoshi_amount += weighted_utxo.utxo.yuv_txout().satoshis; // Track the sum of satoshis

                    Some(weighted_utxo.utxo)
                } else {
                    None
                }
            },
        )
        .collect::<Vec<_>>();

    if satoshi_amount < fee_amount {
        return Err(Error::InsufficientFunds {
            needed: fee_amount,
            available: satoshi_amount,
        });
    }

    Ok(YUVCoinSelectionResult {
        selected,
        fee_amount, // Update fee calculation based on the sum of value
        amount: yuv_amount,
    })
}

#[cfg(test)]
mod test {
    use bitcoin::OutPoint;
    use core::str::FromStr;
    use yuv_pixels::{Luma, Pixel};

    use super::*;
    use crate::types::*;

    // n. of items on witness (1WU) + signature len (1WU) + signature and sighash (72WU)
    // + pubkey len (1WU) + pubkey (33WU) + script sig len (1 byte, 4WU)
    const P2WPKH_SATISFACTION_SIZE: usize = 1 + 1 + 72 + 1 + 33 + 4;

    const FEE_AMOUNT: u64 = 50;

    fn utxo(
        satoshis: u64,
        yuv_amount: u128,
        token: bitcoin::PublicKey,
        index: u32,
    ) -> WeightedUtxo {
        assert!(index < 10);
        let outpoint = OutPoint::from_str(&format!(
            "000000000000000000000000000000000000000000000000000000000000000{}:0",
            index
        ))
        .unwrap();
        WeightedUtxo {
            satisfaction_weight: P2WPKH_SATISFACTION_SIZE,
            utxo: Utxo::Yuv(YuvUtxo {
                outpoint,
                txout: YuvTxOut {
                    satoshis,
                    script_pubkey: Script::new(),
                    pixel: Pixel {
                        luma: Luma::from(yuv_amount as u64),
                        chroma: token.into(),
                    },
                },
                keychain: KeychainKind::External,
                is_spent: false,
                derivation_index: 42,
                confirmation_time: None,
            }),
        }
    }

    fn get_test_utxos() -> Vec<WeightedUtxo> {
        vec![
            utxo(
                100_000,
                500_000,
                bitcoin::PublicKey::from_str(
                    "02ba604e6ad9d3864eda8dc41c62668514ef7d5417d3b6db46e45cc4533bff001c",
                )
                .expect("pubkey"),
                0,
            ),
            utxo(
                FEE_AMOUNT - 40,
                40_000,
                bitcoin::PublicKey::from_str(
                    "02ba604e6ad9d3864eda8dc41c62668514ef7d5417d3b6db46e45cc4533bff001c",
                )
                .expect("pubkey"),
                1,
            ),
            utxo(
                200_000,
                250_000,
                bitcoin::PublicKey::from_str(
                    "02ba604e6ad9d3864eda8dc41c62668514ef7d5417d3b6db46e45cc4533bff001c",
                )
                .expect("pubkey"),
                2,
            ),
        ]
    }

    #[test]
    fn test_largest_first_coin_selection_success() {
        let utxos = get_test_utxos();
        let drain_script = Script::default();
        let target_amount = 600_000;

        let result = YuvLargestFirstCoinSelection
            .coin_select(
                utxos,
                vec![],
                FeeRate::from_sat_per_vb(1.0),
                target_amount,
                &drain_script,
                Chroma::from_str(
                    "ba604e6ad9d3864eda8dc41c62668514ef7d5417d3b6db46e45cc4533bff001c",
                )
                .expect("pubkey"),
            )
            .unwrap();

        assert_eq!(result.selected.len(), 3);
        assert_eq!(result.selected_amount(), 790_000);
        assert_eq!(result.fee_amount, 204)
    }
}
