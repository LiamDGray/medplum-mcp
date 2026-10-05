#![no_main]

use libfuzzer_sys::fuzz_target;
use medplum_mcp_core::audit::AuditEntry;
use medplum_mcp_core::zerocopy_audit::BinaryAuditHeader;
use zerocopy::{FromBytes, IntoBytes};

fuzz_target!(|data: &[u8]| {
    if let Ok(hdr) = BinaryAuditHeader::read_from_bytes(data) {
        assert_eq!(hdr.as_bytes().len(), 120);
        let _ = AuditEntry::from_binary_header(&hdr, "fuzz_tool");
    }
});
