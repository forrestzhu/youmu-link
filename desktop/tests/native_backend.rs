use ymlink_core::files::{FileRequest, FileResponse, MAX_READ_BYTES};
use ymlink_core::session::MAX_PLAINTEXT;
use ymlink_desktop::capture::Display;
use ymlink_desktop::wire::{Message, MirrorAction, MirrorInfo, VERSION};

#[test]
fn application_wire_rejects_unknown_fields_at_both_boundaries() {
    assert!(serde_json::from_str::<Message>(r#"{"kind":"ping","v":4,"extra":1}"#).is_err());
    assert!(serde_json::from_str::<Message>(r#"{"kind":"file_request","v":4,"id":1,"request":{"kind":"stat","share_id":"films","path":"film","extra":1}}"#).is_err());
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
fn mirror_control_schema_roundtrips_and_rejects_unknown_actions_or_fields() {
    assert_eq!(VERSION, 4);
    assert_eq!(VERSION, ymlink_core::session::VERSION);
    let query = Message::MirrorQuery { v: VERSION, id: 1 };
    assert_eq!(
        serde_json::to_value(query).unwrap(),
        serde_json::json!({"kind":"mirror_query","v":4,"id":1})
    );
    for (id, command) in [
        MirrorAction::Pause,
        MirrorAction::Resume,
        MirrorAction::Stop,
        MirrorAction::SelectDisplay { display_id: 7 },
    ]
    .into_iter()
    .enumerate()
    {
        let message = Message::MirrorControl {
            v: VERSION,
            id: id as u64,
            command: command.clone(),
        };
        let mut bytes = vec![];
        ciborium::into_writer(&message, &mut bytes).unwrap();
        assert!(
            matches!(ciborium::from_reader::<Message,_>(bytes.as_slice()).unwrap(),
            Message::MirrorControl { v: VERSION, id: decoded_id, command: decoded_command }
            if decoded_id == id as u64 && decoded_command == command)
        );
    }
    let select = Message::MirrorControl {
        v: VERSION,
        id: 9,
        command: MirrorAction::SelectDisplay { display_id: 7 },
    };
    assert_eq!(
        serde_json::to_value(select).unwrap(),
        serde_json::json!({
            "kind":"mirror_control","v":4,"id":9,"command":{"action":"select_display","display_id":7}
        })
    );
    for rejected in [
        serde_json::json!({"action":"start"}),
        serde_json::json!({"action":"pause","display_id":7}),
        serde_json::json!({"action":"resume","extra":true}),
        serde_json::json!({"action":"stop","extra":true}),
        serde_json::json!({"action":"select_display"}),
        serde_json::json!({"action":"select_display","display_id":-1}),
        serde_json::json!({"action":"select_display","display_id":7,"extra":true}),
    ] {
        assert!(serde_json::from_value::<Message>(serde_json::json!({
            "kind":"mirror_control","v":VERSION,"id":1,"command":rejected
        }))
        .is_err());
    }
    assert!(serde_json::from_value::<Message>(
        serde_json::json!({"kind":"mirror_query","v":VERSION,"id":1,"extra":true})
    )
    .is_err());
}

#[test]
fn mirror_reply_preserves_state_and_denies_nested_unknown_fields() {
    let message = Message::MirrorReply {
        v: VERSION,
        id: 42,
        ok: true,
        error: None,
        state: MirrorInfo {
            status: "paused".into(),
            display_id: Some(7),
            audio: true,
            quality: "balanced".into(),
            error: None,
            displays: vec![Display {
                id: 7,
                name: "Display 7".into(),
                width: 1920,
                height: 1080,
            }],
        },
    };
    let expected = serde_json::to_value(&message).unwrap();
    let mut bytes = vec![];
    ciborium::into_writer(&message, &mut bytes).unwrap();
    let decoded: Message = ciborium::from_reader(bytes.as_slice()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), expected);
    for depth in 0..3 {
        let mut unknown = expected.clone();
        match depth {
            0 => unknown["extra"] = true.into(),
            1 => unknown["state"]["extra"] = true.into(),
            _ => unknown["state"]["displays"][0]["extra"] = true.into(),
        }
        assert!(serde_json::from_value::<Message>(unknown).is_err());
    }
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
