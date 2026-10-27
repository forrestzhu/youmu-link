use super::*;

#[test]
fn mirror_start_announcement_rejects_old_generations_and_full_control_queues() {
    let state = Mutex::new(Snapshot::new("test".into()));
    let current = AtomicU64::new(2);
    let (sender, mut receiver) = mpsc::channel(1);
    assert!(!announce_mirror(&state, &current, 1, &sender, 11));
    assert!(receiver.try_recv().is_err());
    assert!(announce_mirror(&state, &current, 2, &sender, 22));
    current.store(3, Ordering::Release);
    assert!(!announce_mirror(&state, &current, 2, &sender, 11));
    assert!(matches!(
        receiver.try_recv().unwrap(),
        Message::MirrorState { stream_id: 22, .. }
    ));
    sender.try_send(Message::Ping { v: VERSION }).unwrap();
    assert!(!announce_mirror(&state, &current, 3, &sender, 33));
    assert_eq!(state.lock().unwrap().mirror.status, "error");
}

async fn wait_state(backend: &Backend, predicate: impl Fn(&Snapshot) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if predicate(&backend.state.lock().unwrap()) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "backend state transition timed out"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

async fn next(
    receiver: &mut mpsc::Receiver<Message>,
    writer: &mut session::SecureWriter,
) -> Message {
    loop {
        let message = tokio::time::timeout(Duration::from_secs(3), receiver.recv())
            .await
            .expect("response deadline")
            .expect("connection open");
        if let Message::Ping { v: VERSION } = message {
            writer.send(&Message::Pong { v: VERSION }).await.unwrap();
        } else {
            return message;
        }
    }
}

fn receiver(mut reader: session::SecureReader) -> (mpsc::Receiver<Message>, AbortTask) {
    let (send, receive) = mpsc::channel(32);
    let task = AbortTask(tokio::spawn(async move {
        while let Ok(message) = reader.recv::<Message>().await {
            if send.send(message).await.is_err() {
                break;
            }
        }
    }));
    (receive, task)
}

#[test]
fn native_backend_pairs_serves_ranges_controls_transfer_reconnects_and_cancels() {
    let temp = std::env::temp_dir().join(format!("ymlink-runtime-{:016x}", rand::random::<u64>()));
    let root = temp.join("media");
    std::fs::create_dir_all(&root).unwrap();
    let content: Vec<u8> = (0..50_000).map(|i| (i % 251) as u8).collect();
    std::fs::write(root.join("sample.bin"), &content).unwrap();
    let backend = Backend::start_inner(
        temp.join("state"),
        "Test Mac".into(),
        Some(Identity::generate().unwrap()),
        true,
    )
    .unwrap();
    let client = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    client.block_on(async {
        wait_state(&backend,|s|s.listen_port!=0).await;
        backend.command(&json!({"action":"add_share","path":root.to_str().unwrap()}).to_string()).unwrap();
        wait_state(&backend,|s|s.shares.len()==1).await;
        let port=backend.state.lock().unwrap().listen_port;
        let identity=Identity::generate().unwrap();let mut trust=TrustStore::memory();
        let stream=TcpStream::connect(("127.0.0.1",port)).await.unwrap();
        let mut session=SecureSession::handshake(stream,true,&identity,Hello::new(&identity,"Quest test","headset-test",32009)).await.unwrap();
        assert!(session.needs_pairing(&trust).await.unwrap());
        wait_state(&backend,|s|s.pairing.is_some()).await;
        let code=backend.state.lock().unwrap().pairing.as_ref().unwrap().code.clone().unwrap();
        session.authorize(&mut trust,Pairing::Enter{code}).await.unwrap();
        let (read,mut write)=session.into_split().unwrap();let (mut incoming,_read)=receiver(read);
        wait_state(&backend,|s|s.status=="connected").await;
        write.send(&Message::FileRequest{v:VERSION,id:1,request:FileRequest::Shares}).await.unwrap();
        let share=match next(&mut incoming,&mut write).await {Message::FileResponse{id:1,response:FileResponse::Shares{shares},..}=>shares[0].id.clone(),_=>panic!("expected shares")};
        write.send(&Message::FileRequest{v:VERSION,id:2,request:FileRequest::Stat{share_id:share.clone(),path:"sample.bin".into()}}).await.unwrap();
        let version=match next(&mut incoming,&mut write).await {Message::FileResponse{id:2,response:FileResponse::Stat{entry},..}=>{assert_eq!(entry.size,50_000);entry.version.unwrap()},_=>panic!("expected file metadata")};
        write.send(&Message::FileRequest{v:VERSION,id:3,request:FileRequest::Read{share_id:share.clone(),path:"sample.bin".into(),version:version.clone(),offset:7,length:20}}).await.unwrap();
        match next(&mut incoming,&mut write).await {Message::FileResponse{id:3,response:FileResponse::Read{data,offset:7,..},..}=>assert_eq!(data,content[7..27]),_=>panic!("expected range")};
        write.send(&Message::TransferOpen{v:VERSION,id:"download1".into(),share_id:share,path:"sample.bin".into(),version,offset:0}).await.unwrap();
        let offset=match next(&mut incoming,&mut write).await {Message::TransferChunk{offset:0,data,..}=>{assert_eq!(data,content[..data.len()]);data.len()as u64},_=>panic!("expected chunk")};
        backend.command(r#"{"action":"pause_transfer","id":"download1"}"#).unwrap();
        wait_state(&backend,|s|s.transfers[0].status=="paused").await;
        write.send(&Message::TransferAck{v:VERSION,id:"download1".into(),offset}).await.unwrap();
        assert!(matches!(next(&mut incoming,&mut write).await,Message::TransferAction{action,..}if action=="pause"));
        assert!(tokio::time::timeout(Duration::from_millis(60),incoming.recv()).await.is_err(),"paused sender must not send another chunk");
        backend.command(r#"{"action":"resume_transfer","id":"download1"}"#).unwrap();
        assert!(matches!(next(&mut incoming,&mut write).await,Message::TransferAction{action,..}if action=="resume"));
        let acknowledged=match next(&mut incoming,&mut write).await {Message::TransferChunk{offset:next,data,..}=>{assert_eq!(next,offset);assert_eq!(data,content[next as usize..next as usize+data.len()]);next+data.len()as u64},_=>panic!("expected resumed chunk")};
        backend.command(r#"{"action":"cancel_transfer","id":"download1"}"#).unwrap();
        assert!(matches!(next(&mut incoming,&mut write).await,Message::TransferAction{action,..}if action=="cancel"));
        write.send(&Message::TransferAck{v:VERSION,id:"download1".into(),offset:acknowledged}).await.unwrap();
        write.send(&Message::FileRequest{v:VERSION,id:4,request:FileRequest::Shares}).await.unwrap();
        assert!(matches!(next(&mut incoming,&mut write).await,Message::FileResponse{id:4,..}),"late acknowledgement after cancellation must keep the connection usable");
        write.send(&Message::Bye{v:VERSION}).await.unwrap();drop(write);
        wait_state(&backend,|s|s.status!="connected").await;
        let stream=TcpStream::connect(("127.0.0.1",port)).await.unwrap();
        let mut reconnected=SecureSession::handshake(stream,true,&identity,Hello::new(&identity,"Quest test","headset-test",32009)).await.unwrap();
        assert!(!reconnected.needs_pairing(&trust).await.unwrap());
        reconnected.authorize(&mut trust,Pairing::ReconnectOnly).await.unwrap();
        wait_state(&backend,|s|s.status=="connected").await;
        backend.command(&json!({"action":"disconnect","peer_id":identity.device_id()}).to_string()).unwrap();
        wait_state(&backend,|s|s.status!="connected").await;
        drop(reconnected);
        let stream=TcpStream::connect(("127.0.0.1",port)).await.unwrap();
        if let Ok(mut blocked)=SecureSession::handshake(stream,true,&identity,Hello::new(&identity,"Quest test","headset-test",32009)).await {
            assert!(blocked.needs_pairing(&trust).await.is_err(),"manual disconnect must reject the peer's automatic inbound retry");
        }
        backend.command(&json!({"action":"forget","peer_id":identity.device_id()}).to_string()).unwrap();
        wait_state(&backend,|s|!s.peers.iter().any(|p|p.id==identity.device_id())).await;
        let stream=TcpStream::connect(("127.0.0.1",port)).await.unwrap();
        let mut unpaired=SecureSession::handshake(stream,true,&identity,Hello::new(&identity,"Other Quest","headset-test",32010)).await.unwrap();
        assert!(unpaired.needs_pairing(&TrustStore::memory()).await.unwrap());
        wait_state(&backend,|s|s.pairing.is_some()).await;
        backend.command(r#"{"action":"cancel_pairing"}"#).unwrap();
        wait_state(&backend,|s|s.pairing.is_none()).await;
    });
    drop(client);
    drop(backend);
    std::fs::remove_dir_all(temp).unwrap();
}
