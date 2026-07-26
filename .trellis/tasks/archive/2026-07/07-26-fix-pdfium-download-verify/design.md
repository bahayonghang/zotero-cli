# Design

## Trust Model

The source-controlled `PdfiumDownloadTarget` table is the trust anchor. Each entry pins both
the GitHub release archive and the extracted dynamic library. Network metadata is never used to
change expected hashes at runtime. Explicit operator paths remain opt-in; only managed cache
paths gain automatic verification.

The managed filename includes a checksum prefix and the platform library name under the existing
`pdfium-7543` directory. This makes old unverified `pdfium.dll`/`libpdfium.*` files unreachable
from managed discovery and avoids replacing a valid older trust entry during failed updates.

## Installer Pipeline

```text
acquire .install.lock
  -> recheck verified managed library
  -> GET fixed asset URL
  -> bounded stream + SHA-256 into same-dir archive temp + sync
  -> verify archive digest
  -> read only expected regular tar entry into same-dir library temp
  -> cap + sync + verify library digest
  -> remove only an invalid same-name target after all checks pass
  -> atomic persist library temp to final content-addressed path
  -> sync directory where supported
  -> release lock by dropping file handle
```

`NamedTempFile` owns failure cleanup. Archive extraction uses `io::copy`/bounded readers rather
than `tar::Entry::unpack`, so archive paths, links, permissions, and sibling entries cannot choose
the destination.

## Concurrency And Recovery

`fs4::FileExt::lock` uses `flock` on Unix and `LockFileEx` on Windows. The lock file may remain,
but no lock remains after the handle/process exits. A waiting process acquires the lock and
rechecks the verified final path before invoking the downloader, so a successful first installer
eliminates duplicate downloads.

A crash before persist leaves only temporary files. A crash after atomic persist leaves the
fully synced, hash-verified library. An invalid existing content-addressed target is ignored by
discovery and is replaced only after a new artifact has passed both hashes.

## Error Contract

Network/status/read errors remain `ZotError::Remote`; size, checksum, archive shape, and library
verification errors use stable `ZotError::Pdf` codes; local temp/lock/sync/persist failures use
`ZotError::Io` with the failing path. Errors never contain archive bytes or secrets.

## Compatibility And Rollback

- Valid auto-download behavior is unchanged after first verified install; the first run after
  upgrade redownloads once because the old unmanaged filename is no longer trusted.
- The cache root/version directory and P0 candidate order remain unchanged.
- Rollback is the task commit. Old cache files are deliberately preserved, so rollback does not
  require data restoration.
