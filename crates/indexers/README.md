# `yuv-indexers`

Crate for indexers within the YUV node. Currently, it includes a general block indexer that polls Bitcoin RPC for new blocks, and two sub-indexers:

- [Freeze indexer](src/subindexer/freeze.rs): it identifies freeze transactions within blocks.
- [Confirmation indexer](src/subindexer/confirmation.rs): it identifies confirmed transactions within blocks.
