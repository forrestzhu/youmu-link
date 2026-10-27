use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use ymlink_core::identity::{Identity, TrustStore};
use ymlink_core::session::{
    new_pairing_code, Error, Hello, Pairing, SecureSession, MAGIC, MAX_CIPHERTEXT, PAIRING_TTL,
};

async fn sessions(a: &Identity, b: &Identity) -> (SecureSession, SecureSession) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (a, b) = tokio::join!(
        async {
            let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            SecureSession::handshake(stream, true, a, Hello::new(a, "Quest test", "quest", port))
                .await
                .unwrap()
        },
        async {
            let (stream, _) = listener.accept().await.unwrap();
            SecureSession::handshake(stream, false, b, Hello::new(b, "Mac test", "macos", port))
                .await
                .unwrap()
        }
    );
    (a, b)
}

async fn paired(
    a: &Identity,
    b: &Identity,
    a_trust: &mut TrustStore,
    b_trust: &mut TrustStore,
) -> (SecureSession, SecureSession) {
    let (mut a, mut b) = sessions(a, b).await;
    let code = new_pairing_code();
    let (a_result, b_result) = tokio::join!(
        a.authorize(a_trust, Pairing::Enter { code: code.clone() }),
        b.authorize(
            b_trust,
            Pairing::Display {
                code,
                expires_at: Instant::now() + PAIRING_TTL,
            }
        )
    );
    a_result.unwrap();
    b_result.unwrap();
    (a, b)
}

#[tokio::test]
async fn real_tcp_pairing_encrypts_bidirectional_frames_and_reconnects() {
    let a_id = Identity::generate().unwrap();
    let b_id = Identity::generate().unwrap();
    let mut a_trust = TrustStore::memory();
    let mut b_trust = TrustStore::memory();
    let (a, b) = paired(&a_id, &b_id, &mut a_trust, &mut b_trust).await;
    assert_eq!(a.remote().device_id, b_id.device_id());
    assert_eq!(b_trust.peers()[0].device_id, a_id.device_id());
    let (mut a_read, mut a_write) = a.into_split().unwrap();
    let (mut b_read, mut b_write) = b.into_split().unwrap();
    for number in 0..3u32 {
        a_write.send(&number).await.unwrap();
        assert_eq!(b_read.recv::<u32>().await.unwrap(), number);
        b_write.send(&format!("reply {number}")).await.unwrap();
        assert_eq!(
            a_read.recv::<String>().await.unwrap(),
            format!("reply {number}")
        );
    }
    drop((a_read, a_write, b_read, b_write));
    let (mut a, mut b) = sessions(&a_id, &b_id).await;
    let (needs_a, needs_b) = tokio::join!(a.needs_pairing(&a_trust), b.needs_pairing(&b_trust));
    assert!(!needs_a.unwrap());
    assert!(!needs_b.unwrap());
    let (result_a, result_b) = tokio::join!(
        a.authorize(&mut a_trust, Pairing::ReconnectOnly),
        b.authorize(&mut b_trust, Pairing::ReconnectOnly)
    );
    result_a.unwrap();
    result_b.unwrap();
    assert!(a.into_split().is_ok());
    assert!(b.into_split().is_ok());
}

#[tokio::test]
async fn wrong_code_never_opens_business_or_records_trust() {
    let (mut a, mut b) = sessions(
        &Identity::generate().unwrap(),
        &Identity::generate().unwrap(),
    )
    .await;
    let mut a_trust = TrustStore::memory();
    let mut b_trust = TrustStore::memory();
    let (a_result, b_result) = tokio::join!(
        a.authorize(
            &mut a_trust,
            Pairing::Enter {
                code: "000001".into()
            }
        ),
        b.authorize(
            &mut b_trust,
            Pairing::Display {
                code: "000002".into(),
                expires_at: Instant::now() + PAIRING_TTL
            }
        )
    );
    assert!(matches!(a_result, Err(Error::PairingRejected)));
    assert!(matches!(b_result, Err(Error::PairingRejected)));
    assert!(a_trust.peers().is_empty());
    assert!(b_trust.peers().is_empty());
    assert!(matches!(a.into_split(), Err(Error::PairingRequired)));
    assert!(matches!(b.into_split(), Err(Error::PairingRequired)));
}

#[tokio::test]
async fn one_sided_trust_cannot_bypass_pairing() {
    let (mut a, mut b) = sessions(
        &Identity::generate().unwrap(),
        &Identity::generate().unwrap(),
    )
    .await;
    let mut a_trust = TrustStore::memory();
    let mut b_trust = TrustStore::memory();
    a_trust.remember(a.remote(), &a.remote_key()).unwrap();
    let (needs_a, needs_b) = tokio::join!(a.needs_pairing(&a_trust), b.needs_pairing(&b_trust));
    assert!(needs_a.unwrap());
    assert!(needs_b.unwrap());
    assert!(matches!(
        a.authorize(&mut a_trust, Pairing::ReconnectOnly).await,
        Err(Error::PairingRequired)
    ));
    assert!(matches!(
        b.authorize(&mut b_trust, Pairing::ReconnectOnly).await,
        Err(Error::PairingRequired)
    ));
    assert!(matches!(a.into_split(), Err(Error::PairingRequired)));
}

#[tokio::test]
async fn revoked_trust_cannot_use_a_cached_reconnect_grant() {
    let a_id = Identity::generate().unwrap();
    let b_id = Identity::generate().unwrap();
    let mut a_trust = TrustStore::memory();
    let mut b_trust = TrustStore::memory();
    drop(paired(&a_id, &b_id, &mut a_trust, &mut b_trust).await);
    let (mut a, mut b) = sessions(&a_id, &b_id).await;
    let (needs_a, needs_b) = tokio::join!(a.needs_pairing(&a_trust), b.needs_pairing(&b_trust));
    assert!(!needs_a.unwrap());
    assert!(!needs_b.unwrap());
    a_trust.forget(&b_id.device_id()).unwrap();
    assert!(matches!(
        a.authorize(&mut a_trust, Pairing::ReconnectOnly).await,
        Err(Error::PairingRequired)
    ));
    assert!(matches!(a.into_split(), Err(Error::PairingRequired)));
}

#[tokio::test]
async fn pairing_expires_while_waiting_for_input() {
    let (mut a, mut b) = sessions(
        &Identity::generate().unwrap(),
        &Identity::generate().unwrap(),
    )
    .await;
    let a_trust = TrustStore::memory();
    let mut b_trust = TrustStore::memory();
    let (needs_a, needs_b) = tokio::join!(a.needs_pairing(&a_trust), b.needs_pairing(&b_trust));
    assert!(needs_a.unwrap());
    assert!(needs_b.unwrap());
    let result = b
        .authorize(
            &mut b_trust,
            Pairing::Display {
                code: "001234".into(),
                expires_at: Instant::now() + Duration::from_millis(10),
            },
        )
        .await;
    assert!(matches!(result, Err(Error::PairingExpired)));
    assert!(b_trust.peers().is_empty());
    assert!(matches!(b.into_split(), Err(Error::PairingRequired)));
}

#[tokio::test]
async fn cancelled_receive_cannot_restart_in_the_middle_of_a_frame() {
    let (a, b) = paired(
        &Identity::generate().unwrap(),
        &Identity::generate().unwrap(),
        &mut TrustStore::memory(),
        &mut TrustStore::memory(),
    )
    .await;
    let (mut reader, _) = a.into_split().unwrap();
    let (_, mut writer) = b.into_split().unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(10), reader.recv::<String>())
            .await
            .is_err()
    );
    writer.send(&"late frame").await.unwrap();
    assert!(matches!(
        reader.recv::<String>().await,
        Err(Error::Protocol("receive direction is closed"))
    ));
}

#[tokio::test]
async fn forged_device_id_is_rejected_by_authenticated_noise_key() {
    let a = Identity::generate().unwrap();
    let b = Identity::generate().unwrap();
    let old_b = Identity::generate().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (a_result, _) = tokio::join!(
        async {
            let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            SecureSession::handshake(stream, true, &a, Hello::new(&a, "Quest", "quest", port)).await
        },
        async {
            // Bypass our local Hello validation to act as an actual malicious
            // peer: authenticate a new Noise key but claim the previous id.
            let (mut stream, _) = listener.accept().await.unwrap();
            let params = "Noise_XX_25519_ChaChaPoly_SHA256".parse().unwrap();
            let keypair = snow::Builder::new(params).generate_keypair().unwrap();
            let mut handshake =
                snow::Builder::new("Noise_XX_25519_ChaChaPoly_SHA256".parse().unwrap())
                    .prologue(b"youmu-link-v2")
                    .local_private_key(&keypair.private)
                    .build_responder()
                    .unwrap();
            let mut magic = [0u8; 9];
            stream.read_exact(&mut magic).await.unwrap();
            assert_eq!(&magic, MAGIC);
            let mut output = vec![0u8; MAX_CIPHERTEXT];
            while !handshake.is_handshake_finished() {
                if handshake.is_my_turn() {
                    let size = handshake.write_message(&[], &mut output).unwrap();
                    stream.write_u32(size as u32).await.unwrap();
                    stream.write_all(&output[..size]).await.unwrap();
                } else {
                    let size = stream.read_u32().await.unwrap() as usize;
                    assert!(size <= 4096);
                    let mut input = vec![0u8; size];
                    stream.read_exact(&mut input).await.unwrap();
                    handshake.read_message(&input, &mut output).unwrap();
                }
            }
            let mut transport = handshake.into_transport_mode().unwrap();
            let size = stream.read_u32().await.unwrap() as usize;
            assert!(size <= MAX_CIPHERTEXT);
            let mut input = vec![0u8; size];
            stream.read_exact(&mut input).await.unwrap();
            transport.read_message(&input, &mut output).unwrap();
            let mut forged = Hello::new(&b, "Mac", "macos", port);
            forged.public_key = keypair.public;
            forged.device_id = old_b.device_id();
            let mut plaintext = vec![];
            ciborium::into_writer(&forged, &mut plaintext).unwrap();
            let size = transport.write_message(&plaintext, &mut output).unwrap();
            stream.write_u32(size as u32).await.unwrap();
            stream.write_all(&output[..size]).await.unwrap();
        }
    );
    assert!(matches!(a_result, Err(Error::IdentityChanged)));
    let mut trust = TrustStore::memory();
    let old = Hello::new(&old_b, "Mac", "macos", port);
    trust.remember(&old, &old_b.public_key()).unwrap();
    assert!(matches!(
        trust.check(&old, &b.public_key()),
        Err(Error::IdentityChanged)
    ));
}

#[tokio::test]
async fn oversized_frame_is_rejected_before_allocation_and_large_send_keeps_nonce() {
    let identity = Identity::generate().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (server, ()) = tokio::join!(
        async {
            let (stream, _) = listener.accept().await.unwrap();
            SecureSession::handshake(
                stream,
                false,
                &identity,
                Hello::new(&identity, "Mac", "macos", port),
            )
            .await
        },
        async {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            stream.write_all(MAGIC).await.unwrap();
            stream.write_u32(4097).await.unwrap();
        }
    );
    assert!(matches!(
        server,
        Err(Error::Protocol("invalid frame length"))
    ));

    let (a, b) = paired(
        &Identity::generate().unwrap(),
        &Identity::generate().unwrap(),
        &mut TrustStore::memory(),
        &mut TrustStore::memory(),
    )
    .await;
    let (_, mut writer) = a.into_split().unwrap();
    let (mut reader, _) = b.into_split().unwrap();
    assert!(matches!(
        writer.send(&vec![0u8; MAX_CIPHERTEXT]).await,
        Err(Error::Protocol(_))
    ));
    writer.send(&"after rejected oversize").await.unwrap();
    assert_eq!(
        reader.recv::<String>().await.unwrap(),
        "after rejected oversize"
    );
}

#[test]
fn trust_roundtrip_is_private_and_corruption_fails_closed() {
    let directory =
        std::env::temp_dir().join(format!("ymlink-trust-{:016x}", rand::random::<u64>()));
    let path = directory.join("trust.json");
    let identity = Identity::generate().unwrap();
    let hello = Hello::new(&identity, "Mac", "macos", 47620);
    let mut trust = TrustStore::load(&path).unwrap();
    trust
        .remember_endpoint(
            &hello,
            &identity.public_key(),
            Some("127.0.0.1:47620".into()),
        )
        .unwrap();
    let replacement = Identity::generate().unwrap();
    let replacement_hello = Hello::new(&replacement, "Mac", "macos", 47620);
    assert!(matches!(
        trust.check_endpoint(
            &replacement_hello,
            &replacement.public_key(),
            "127.0.0.1:47620"
        ),
        Err(Error::IdentityChanged)
    ));
    let mut reloaded = TrustStore::load(&path).unwrap();
    assert!(reloaded.check(&hello, &identity.public_key()).unwrap());
    assert_eq!(
        reloaded.peers()[0].endpoint.as_deref(),
        Some("127.0.0.1:47620")
    );
    let mut detached = reloaded.detached();
    detached.forget(&identity.device_id()).unwrap();
    assert!(detached.peers().is_empty());
    assert!(reloaded.check(&hello, &identity.public_key()).unwrap());
    assert!(TrustStore::load(&path)
        .unwrap()
        .check(&hello, &identity.public_key())
        .unwrap());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    reloaded.forget(&identity.device_id()).unwrap();
    assert!(TrustStore::load(&path).unwrap().peers().is_empty());
    assert!(!reloaded
        .check_endpoint(
            &replacement_hello,
            &replacement.public_key(),
            "127.0.0.1:47620",
        )
        .unwrap());
    std::fs::write(&path, b"corrupt").unwrap();
    assert!(TrustStore::load(&path).is_err());
    std::fs::remove_dir_all(directory).unwrap();
}
