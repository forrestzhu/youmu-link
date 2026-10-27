# Discovery v1 固定向量

本目录可由未来 Kotlin 实现共享，只有公开协议常量：tag `youmu-link`、nonce `00` 到 `0f`、端口 `47620`、能力 `control` / `files` / `mirror`。不含真实设备身份、地址、名称、路径或密钥；仓库的私有政策不变。

`SHA256("youmu-link")[0..16]` 为 `a3dafa8fb726739261f89f9279f2a942`；beacon 共 133 字节，query 共 84 字节，均小于 512B 限制。

- `semantics.json`：预期解码语义。
- `beacon-v1.hex` / `query-v1.hex`：固定 CBOR 字节，允许去除行间空白后读取。
- `../../protocol_vectors.rs`：Rust 读取固定向量、核对语义与重新编码后的语义，并验证倒置 map 键顺序仍可解码。运行 `cargo test --offline -p ymlink-core --test protocol_vectors`。

向量按 [RFC 8949 §3.1](https://www.rfc-editor.org/rfc/rfc8949.html#section-3.1) 手工拼接 map / text / uint / array / bool；tag 指纹另外用 Python 标准库 SHA-256 计算，未调用生产 Rust 编码器生成 `.hex`。

结构核对：beacon 为 `a6`（6 对 map），query 为 `a4`（4 对 map）；`61 76 01` 为 `v:1`；`19 ba 04` 为端口 `47620`；`83` 为 3 项能力数组；`f5` 为 true。两个 16 元素数组的头均为 `90`，nonce 后续为 `00 01 ... 0f`；hash 元素小于 24 时直接编码，其余使用 `18 xx`。

当前 Rust `Vec<u8>` 将 `tag_hash` 与 `session_nonce` 编码为 CBOR **无符号整数数组**（major type 4，元素 0–255），不是 major type 2 的 byte string。其它实现应能解码这里的实际数组格式；本夹具不新增其它编码形式的接收要求。map 键顺序不作统一要求，不要求不同语言编码得到相同字节序；比较解码后的字段语义。

Rust 读取成功只补齐共享夹具和 L1 / L2 证据；Kotlin 尚未对拍，L4 互操作仍为 **not-run**。发现版本仍为 1，本次未更改任何 wire 字段或版本。
