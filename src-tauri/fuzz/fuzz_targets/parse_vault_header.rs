#![no_main]

use dynamic_markdown_dock_lib::crypto::vault_format::VaultHeader;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // VaultHeader::parse must never panic or OOM on arbitrary untrusted bytes.
    let _ = VaultHeader::parse(data);
});
