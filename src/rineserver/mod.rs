pub mod handles;
pub mod objects;
pub mod protocol;
pub mod server;

pub use handles::HandleTable;
pub use objects::KernelObject;
pub use protocol::{RineServerReply, RineServerRequest};
pub use server::RineServer;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::STATUS_SUCCESS;

    #[test]
    fn test_rineserver_handle_lifecycle() {
        let server = RineServer::get_instance();
        let reply = server.dispatch(RineServerRequest::CreateEvent {
            manual_reset: true,
            initial_state: false,
            name: "TestEvent".into(),
        });

        if let RineServerReply::HandleCreated { handle, status } = reply {
            assert_eq!(status, STATUS_SUCCESS);
            assert!(handle >= 4);

            let set_reply = server.dispatch(RineServerRequest::SetEvent { handle });
            if let RineServerReply::EventStatus { status } = set_reply {
                assert_eq!(status, STATUS_SUCCESS);
            } else {
                panic!("SetEvent failed");
            }

            let close_reply = server.dispatch(RineServerRequest::CloseHandle { handle });
            if let RineServerReply::HandleClosed { status } = close_reply {
                assert_eq!(status, STATUS_SUCCESS);
            } else {
                panic!("CloseHandle failed");
            }
        } else {
            panic!("CreateEvent failed");
        }
    }
}
