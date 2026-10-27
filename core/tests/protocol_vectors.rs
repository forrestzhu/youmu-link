//! Shared discovery vectors are fixed independently of the Rust encoder.
use ciborium::value::Value;
use serde::Deserialize;
use ymlink_core::beacon::{self, Beacon, Packet, Query};

#[derive(Deserialize)]
struct Semantics {
    tag: String,
    beacon: Beacon,
    query: Query,
}

fn from_hex(input: &str) -> Vec<u8> {
    let hex: String = input.split_whitespace().collect();
    assert_eq!(hex.len() % 2, 0);
    hex.as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn fixed_discovery_v1_vectors_match_semantics_and_allow_different_map_order() {
    let semantics: Semantics =
        serde_json::from_str(include_str!("fixtures/discovery-v1/semantics.json")).unwrap();
    let nonce: Vec<u8> = (0..16).collect();
    assert_eq!(semantics.tag, "youmu-link");
    assert_eq!(semantics.beacon.session_nonce, nonce);
    assert_eq!(semantics.query.session_nonce, nonce);
    assert_eq!(semantics.beacon.tag_hash, beacon::tag_hash(&semantics.tag));
    assert_eq!(semantics.query.tag_hash, semantics.beacon.tag_hash);
    assert_eq!(semantics.beacon.listen_port, 47620);
    assert_eq!(semantics.beacon.caps, ["control", "files", "mirror"]);
    assert!(semantics.beacon.discoverable);
    assert!(semantics.query.query);

    for (hex, expected) in [
        (
            include_str!("fixtures/discovery-v1/beacon-v1.hex"),
            Packet::Beacon(semantics.beacon),
        ),
        (
            include_str!("fixtures/discovery-v1/query-v1.hex"),
            Packet::Query(semantics.query),
        ),
    ] {
        let bytes = from_hex(hex);
        assert!(bytes.len() <= beacon::MAX_PACKET);
        let decoded = beacon::decode_packet(&bytes).unwrap();
        assert_eq!(decoded, expected);
        let reencoded = match &decoded {
            Packet::Beacon(value) => value.encode().unwrap(),
            Packet::Query(value) => value.encode().unwrap(),
        };
        assert_eq!(beacon::decode_packet(&reencoded).unwrap(), expected);

        // Vec<u8> currently encodes as uint arrays, not CBOR byte strings.
        let Value::Map(mut fields) = ciborium::from_reader::<Value, _>(bytes.as_slice()).unwrap()
        else {
            panic!("discovery vector must be a map");
        };
        for name in ["tag_hash", "session_nonce"] {
            let value = &fields
                .iter()
                .find(|(key, _)| key.as_text() == Some(name))
                .unwrap()
                .1;
            let Value::Array(octets) = value else {
                panic!("{name} must be a uint array")
            };
            assert_eq!(octets.len(), 16);
            assert!(octets
                .iter()
                .all(|value| matches!(value, Value::Integer(n) if u8::try_from(*n).is_ok())));
        }
        // Map order is not a cross-language wire requirement.
        fields.reverse();
        let mut reordered = Vec::new();
        ciborium::into_writer(&Value::Map(fields), &mut reordered).unwrap();
        assert_eq!(beacon::decode_packet(&reordered).unwrap(), expected);
    }
}
