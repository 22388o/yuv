
<h1 align="center">
    Bulletproof
</h1>

This crate provides a pure Rust implementation of [Bulletproof Plus](https://eprint.iacr.org/2020/735.pdf) with 64-bit range proof support.

## Bulletproofs in action

```rust
use rand::RngCore;

fn main() {
    let value = 200u64;

    let mut blinding = [0u8; 32];

    let mut rng = rand::thread_rng();
    rng.fill_bytes(&mut blinding[..]);

    let (proof, commit) = bulletproof::generate(value, blinding);

    assert!(bulletproof::verify(commit, proof));
}
```

## Licence

Licensed under [Apache Licence, Version 2.0](../../LICENSE)
