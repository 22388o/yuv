# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] - 2024-22-05

### Fixed

- Move the messages about failing to retrieve the block in the blockloader to the warn log level.
- Add the check to the `AnnouncementIndexer` if the `OP_RETURN` isn't an announcement message to not
  spam with error messages.
- Update the handler to properly handle issuance transactions and avoid collisions between RPC and
  indexer.
- Move tx confirmation to a separate crate.
- Add event about an announcement message is checked to the `Controller`.
- Zero amount proofs are skipped at check step.
- Fix missing witness data in issue transaction inputs while drain tweaked satoshis.
- Fix the YUV node's connection to itself due to unfiltered P2P's `Addr` message.
- Fix the waste of satoshis on `OP_RETURN` for announcements.
- Add bootnode for `Mainnet` and `Mutiny Testnet` (more to come in a few days).

### Added

- Add the duration restriction of the end-to-end test to the configuration file.
- Add a bitcoin blocks mining to the end-to-end test.
- Add a custom Network type we can further use to add custom networks.
- Add support for `Mutiny` network.
- Add a list of hardcoded Mutiny bootnodes.
- Add the ability to send announcement messages with Esplora `bitcoin-provider` in YUV CLI.
