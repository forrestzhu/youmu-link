use ymlink_core::files::{FileRequest, FileResponse, MAX_READ_BYTES};
use ymlink_core::session::MAX_PLAINTEXT;
use ymlink_desktop::wire::{Message, VERSION};

#[test]
fn application_wire_rejects_unknown_fields_at_both_boundaries() {
    assert!(serde_json::from_str::<Message>(r#"{"kind":"ping","v":2,"extra":1}"#).is_err());
    assert!(serde_json::from_str::<Message>(r#"{"kind":"file_request","v":2,"id":1,"request":{"kind":"stat","share_id":"films","path":"film","extra":1}}"#).is_err());
    let request = Message::FileRequest {
        v: VERSION,
        id: 42,
        request: FileRequest::List {
            share_id: "films".into(),
            path: "".into(),
            cursor: None,
            limit: 256,
        },
    };
    let encoded = serde_json::to_vec(&request).unwrap();
    assert!(matches!(
        serde_json::from_slice::<Message>(&encoded).unwrap(),
        Message::FileRequest {
            v: VERSION,
            id: 42,
            request: FileRequest::List { limit: 256, .. }
        }
    ));
}

#[test]
fn largest_transfer_and_file_chunks_fit_an_encrypted_frame() {
    let data = vec![255; MAX_READ_BYTES as usize];
    let messages = [
        Message::TransferChunk {
            v: VERSION,
            id: "t".repeat(64),
            offset: u64::MAX,
            total: u64::MAX,
            version: "f".repeat(64),
            data: data.clone(),
            eof: false,
        },
        Message::FileResponse {
            v: VERSION,
            id: u64::MAX,
            response: FileResponse::Read {
                offset: u64::MAX,
                total: u64::MAX,
                version: "f".repeat(64),
                data,
                eof: false,
            },
        },
        Message::Media {
            v: VERSION,
            stream_id: u64::MAX,
            packet_id: u64::MAX,
            part: 2047,
            parts: 2048,
            media_kind: "video".into(),
            codec: "h264-avcc".into(),
            pts_us: u64::MAX,
            keyframe: true,
            width: 3840,
            height: 2160,
            data: vec![255; 8192],
        },
    ];
    for message in messages {
        let mut encoded = Vec::new();
        ciborium::into_writer(&message, &mut encoded).unwrap();
        assert!(encoded.len() < MAX_PLAINTEXT);
        let decoded: Message = ciborium::from_reader(encoded.as_slice()).unwrap();
        assert_eq!(
            serde_json::to_value(decoded).unwrap(),
            serde_json::to_value(message).unwrap()
        );
    }
}
