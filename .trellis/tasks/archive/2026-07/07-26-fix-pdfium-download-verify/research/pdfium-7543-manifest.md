# Pdfium Chromium 7543 Manifest Research

Source queried 2026-07-26:

`https://api.github.com/repos/bblanchon/pdfium-binaries/releases/tags/chromium%2F7543`

The GitHub release API `digest` values were cross-checked by downloading each currently supported
asset. The extracted library hashes were computed from the exact manifest path in each archive.

| Asset | Archive bytes | Archive SHA-256 | Library bytes | Library SHA-256 |
|---|---:|---|---:|---|
| `pdfium-win-x64.tgz` | 3075408 | `0b08b606792a6cc593426efdefc6622611bce446d9e0270743846956ea1554ca` | 5787136 | `6b963c2be9cacbaa0c0c7f4bf6d20d2fd16729ebdaa9989978b0f7b119c1c1cb` |
| `pdfium-win-arm64.tgz` | 2917957 | `bb4a00113494e25bbee52d3d63b7f4ecf0de2d277b7de75ba9a1d5b987a74509` | 5365760 | `368986d82c11a22e0c53728873899cf864dbd7b32a42214a660ac30fe8ba37f4` |
| `pdfium-win-x86.tgz` | 2967971 | `25c635e70037c6a20a33126a812a63e891c70974982a2e00112b7aaa07eb3832` | 5357056 | `51db7685cc3c9ee11bc4c101d44b4ba30cb11c911c31c5c6da79c5bea0d76ffa` |
| `pdfium-mac-x64.tgz` | 2899900 | `2510460ac106f14b884598a0da3f53a99e23d79512acf027c5e101c2bb2f26cb` | 5804728 | `c4ae7ca1583e04449d07f1985ce258a3f935583279fd46fa89f528106301b929` |
| `pdfium-mac-arm64.tgz` | 2781265 | `41c269723b4711793de70ff34e65c00fa79907d6c023741837579e906b846faa` | 5530976 | `858f0676a1ac5b666673fc6e56b4401f95907a3fc66fa4635d626097a04c205b` |
| `pdfium-linux-x64.tgz` | 2957508 | `9329a3c4b19b3c8d0a93af5440f44be84e4bd879a204e47b1a7a160e96809da4` | 5996800 | `2383a414050dd21ae5300b119ad8a72360ef92cff820b4c685c047dc272c2794` |
| `pdfium-linux-arm64.tgz` | 2942365 | `4965a4c0b64c45b5edefa1072e2b483bf90b4d25d7deec44f104dcbdecf05c3e` | 6185752 | `deab139b06cba02552d0d695eb4789600da41a2df9d9176f3ec5ce477bff53a8` |

Implementation dependency research:

- `sha2 0.10.9`: RustCrypto pure Rust SHA-2 implementation, MIT OR Apache-2.0.
- `fs4 1.1.0`: pure Rust cross-platform locks using `flock`/`LockFileEx`, MSRV 1.75,
  MIT OR Apache-2.0. Locks release when the file handle/process exits.

The release also publishes `attestation.json`, but online attestation/provenance verification is
part of the parent task's excluded L-03 release-engineering scope. This task uses reviewed,
source-controlled digest pins and does not claim signature/provenance verification.
