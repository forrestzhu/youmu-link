use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::time::timeout;
use ymlink_core::files::{FileRequest, FileResponse};
use ymlink_core::identity::{Identity, TrustStore};
use ymlink_core::session::{Hello, Pairing, SecureSession};
use ymlink_desktop::wire::{Message, VERSION};

struct Stub(Option<Child>);

impl Drop for Stub {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[tokio::test]
async fn requests_a_pairing_window_before_waiting_for_stdin() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut stub = Stub(Some(
        Command::new(env!("CARGO_BIN_EXE_headset_stub"))
            .args([format!("127.0.0.1:{port}"), "shares".into()])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    ));
    timeout(Duration::from_secs(4), async {
        let identity = Identity::generate().unwrap();
        let (socket, _) = listener.accept().await.unwrap();
        let mut session = SecureSession::handshake(
            socket,
            false,
            &identity,
            Hello::new(&identity, "Desktop test", "windows", port),
        )
        .await
        .unwrap();
        let mut trust = TrustStore::memory();
        assert!(session.needs_pairing(&trust).await.unwrap());
        // No input has been written: the display must be able to offer its code first.
        let resume = timeout(Duration::from_secs(2), session.receive_pairing_request())
            .await
            .expect("headset_stub must request a pairing window while stdin is pending")
            .unwrap();
        assert!(resume.is_none());
        let expires_at = Instant::now() + Duration::from_secs(30);
        session.offer_pairing([7; 16], expires_at).await.unwrap();
        writeln!(stub.0.as_mut().unwrap().stdin.as_mut().unwrap(), "0427").unwrap();
        session
            .authorize(
                &mut trust,
                Pairing::Display {
                    code: "0427".into(),
                    expires_at,
                },
            )
            .await
            .unwrap();
        let (mut reader, mut writer) = session.into_split().unwrap();
        let id = match reader.recv::<Message>().await.unwrap() {
            Message::FileRequest {
                v: VERSION,
                id,
                request: FileRequest::Shares,
            } => id,
            message => panic!("expected shares request, got {message:?}"),
        };
        writer
            .send(&Message::FileResponse {
                v: VERSION,
                id,
                response: FileResponse::Shares { shares: vec![] },
            })
            .await
            .unwrap();
        assert!(matches!(
            reader.recv::<Message>().await.unwrap(),
            Message::Bye { v: VERSION }
        ));
        while stub.0.as_mut().unwrap().try_wait().unwrap().is_none() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("headset_stub pairing and shares exchange timed out");
    let output = stub.0.take().unwrap().wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "headset_stub exited unsuccessfully"
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
        serde_json::json!({"kind": "shares", "shares": []})
    );
}
