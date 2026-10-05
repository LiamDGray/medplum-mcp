#![no_main]

use libfuzzer_sys::fuzz_target;
use medplum_mcp_server::client::MedplumClient;
use medplum_mcp_server::mcp::McpServer;

fuzz_target!(|data: &[u8]| {
    if let Ok(msg_str) = std::str::from_utf8(data) {
        let client = MedplumClient::new_demo_default();
        let server = McpServer::new(client, None, false);

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let _ = server.handle_jsonrpc_message(msg_str).await;
        });
    }
});
