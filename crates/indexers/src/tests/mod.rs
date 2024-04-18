use std::{collections::HashSet, sync::Arc, time::Duration};

use async_trait::async_trait;
use bitcoin::BlockHash;
use bitcoin_client::{
    json::{self, GetBlockTxResult},
    MockRpcApi,
};
use event_bus::{Event, EventBus};
use mockall::predicate;
use once_cell::sync::Lazy;
use tokio_util::sync::CancellationToken;
use yuv_storage::{BlockIndexerStorage, LevelDB};

use crate::Subindexer;
use crate::{BitcoinBlockIndexer, RunParams};

const POLLING_PERIOD: Duration = Duration::from_secs(1);
const SLEEP_TIME: Duration = Duration::from_secs(1);

const ZERO_BLOCK_INDEX: usize = 0;
const BEST_BLOCK_INDEX: usize = 3;

static BLOCKS: Lazy<Vec<json::GetBlockTxResult>> = Lazy::new(|| {
    vec![
        json_to_block(include_str!("./assets/zero_block.json")),
        json_to_block(include_str!("./assets/first_block.json")),
        json_to_block(include_str!("./assets/second_block.json")),
        json_to_block(include_str!("./assets/best_block.json")),
    ]
});

fn json_to_block(json: &str) -> GetBlockTxResult {
    serde_json::from_str::<GetBlockTxResult>(json).expect("JSON was not well-formatted")
}

#[derive(Clone, Event)]
enum IndexerTestMessage {
    /// Message that signals that the block was handled.
    HandleBlock { block: GetBlockTxResult },
}

struct TestIndexer {
    event_bus: EventBus,
}

impl TestIndexer {
    fn new(event_bus: EventBus) -> Self {
        TestIndexer { event_bus }
    }
}

#[async_trait]
impl Subindexer for TestIndexer {
    async fn index(&mut self, block: &GetBlockTxResult) -> eyre::Result<()> {
        self.event_bus
            .send(IndexerTestMessage::HandleBlock {
                block: block.clone(),
            })
            .await;

        Ok(())
    }
}

/// This test checks if all the blocks are indexed when the block generation period is close to immediate.
/// This situation is most likely to occur in regtest.
#[tokio::test]
async fn test_indexer_handles_immediately_generated_blocks() {
    let mut btc_client_mock = MockRpcApi::new();
    let mut event_bus = EventBus::default();

    event_bus.register::<IndexerTestMessage>(None);
    let blocks_receiver = event_bus.subscribe::<IndexerTestMessage>();

    // Getting the best block (last block).
    btc_client_mock
        .expect_get_best_block_hash()
        .times(..)
        .returning(|| Ok(BLOCKS[BEST_BLOCK_INDEX].block_data.hash));

    for block in BLOCKS.clone() {
        // Each block is expected to be fetched once.
        add_expect_get_block_txs(&mut btc_client_mock, block);
    }

    let storage = LevelDB::in_memory().expect("failed to initialize storage");

    storage
        .put_last_indexed_hash(BLOCKS[ZERO_BLOCK_INDEX].block_data.hash)
        .await
        .expect("failed to put last indexed hash");

    let mut indexer = BitcoinBlockIndexer::new(Arc::new(btc_client_mock), storage);

    // Adding a test indexer that will send indexed blocks back.
    indexer.add_indexer(TestIndexer::new(event_bus));

    tokio::spawn(indexer.run(
        RunParams {
            polling_period: POLLING_PERIOD,
        },
        CancellationToken::default(),
    ));
    tokio::time::sleep(SLEEP_TIME).await;

    let mut received_blocks: HashSet<BlockHash> = HashSet::new();
    for _ in 0..blocks_receiver.len() {
        let event = blocks_receiver.recv().await.expect("channel must be alive");
        let IndexerTestMessage::HandleBlock { block } = event;
        received_blocks.insert(block.block_data.hash);
    }

    // Check that all the blocks were indexed.
    for block in BLOCKS.clone() {
        assert!(
            received_blocks.contains(&block.block_data.hash),
            "some block was missed during indexing"
        );
    }
}

fn add_expect_get_block_txs(rpc_api: &mut MockRpcApi, block: GetBlockTxResult) {
    rpc_api
        .expect_get_block_txs()
        .with(predicate::eq(block.block_data.hash))
        .once()
        .returning(move |_| Ok(block.clone()));
}
