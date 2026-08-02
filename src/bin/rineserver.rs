use rine::rineserver::RineServer;

fn main() {
    println!("[RINESERVER DAEMON] Starting RineServer Kernel IPC Daemon (Replacing WineServer)...");
    let server = RineServer::get_instance();
    let status = server.dispatch(rine::rineserver::RineServerRequest::QueryServerStatus);

    if let rine::rineserver::RineServerReply::ServerStatusResponse { active_handles, .. } = status {
        println!(
            "[RINESERVER DAEMON] Kernel Server active and listening. Initial handles in table: {}",
            active_handles
        );
    }
    println!("[RINESERVER DAEMON] RineServer IPC daemon initialized successfully.");
}
