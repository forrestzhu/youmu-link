# 历史提交邮箱修正

日期：2026-10-27。任务：[YML-004](../tasks/archive/YML-004-opensource-ci.md)。用户明确要求修正历史公司邮箱，目标沿用 `jayzero@foxmail.com`。本轮授权覆盖该元数据改写及更新 main 所需的精确 lease 推送；不扩大为历史正文清理或仓库公开。

## 实施与验证

- 开工 main 干净，fetch / ff-only 无新增；改写前本机和远端 main 均为 `c474d197e0edbcf0ffe766a888c4b0d699115fcf`。20 个既有提交中，author / committer 各有 10 个目标邮箱、10 个同一公司邮箱，无 merge / commit 签名。原公司邮箱未复制到报告。
- 改写前以 `git bundle create --all` 备份全部本地 refs。备份无前置依赖，`git bundle verify`、pack 完整性与 SHA-256 经根代理 / 独立审查核验；只保留在忽略入库的本机 `dist/history-email-rewrite`，不提交、不推送备份 ref。
- 在隔离 bare 仓库中使用 Git 原生 `cat-file` / `hash-object`，仅替换精确旧邮箱的 author / committer header，并按顺序映射 parent；不安装新依赖、不改姓名、日期、提交说明、tree 或文件内容。
- 根代理与独立 subagent `history_email_review` 分别重建并验证 20/20 raw commits。20/20 tree、姓名、日期、message 保持不变，parent 全部按映射对应；author / committer 各修正 10 项。最早 4 个 SHA 不变，后 16 个变化。独立审查提前算出的新 tip 与实际结果相同。
- 新 tip 为 `86d19aef907596d57c2b80e5e8312292ab8cea1f`，新旧 tip tree 都是 `7d4c070151e81120e861652f0fb51749c062adc2`。候选 `git fsck --full --strict` 通过，原工作区不经 hard reset 直接更新到同树的新历史，文件修改为 0。
- 对 main 使用 `--force-with-lease=refs/heads/main:c474d197e0edbcf0ffe766a888c4b0d699115fcf`，只推送 `HEAD:refs/heads/main`；推送前后回读远端 tip。未使用无条件 force 或 mirror，未改其他远端 refs。
- 推送后从 GitHub 独立 bare clone，仅获取 main / 无 tags，实际核验 20 个可达提交：author 20/20、committer 20/20 均为目标邮箱，20/20 raw commits 与候选一致，HEAD tree 不变，`fsck --full --strict` 通过。随后用正常文档提交记录本次结果，交付再验证远端与本地 main 一致。

第二轮独立只读复核覆盖 8 个已有文件 diff、新报告、20 项映射和远端 clone，确认仅追加文档 / 授权记录，无代码改动；`harness check` / `git diff --check` 通过，无新增问题。两轮核验均由 `history_email_review` 独立于实施者完成。

备份 bundle SHA-256：`1f4bff07103ec848ca298651f6efea740725e164c3e1d5275fac6d98d0a28ce6`。备份含旧邮箱，属于本机恢复材料，不是公开制品。

本轮仅修改 Git 元数据并追加仓库记录，没有应用 / 协议代码改动，不重复运行 cargo / 原生构建；运行 `harness check` 检查新增记录 / 链接，交付运行 `check --require-pushed`。已有制品的 SHA-256、blob / tree ID 和 BUILD-INFO 不改写；其旧提交号按下表对应，不能把旧包重新标作新提交构建。

## 旧 → 新提交号

以下保留改写前提交号作为历史证据引用的索引。原报告、Actions run 和既有制品中的旧号不批量替换；后续实现以当前 main 为准。

| 改写前 | 改写后 |
| --- | --- |
| `01b329f5c54be12d36c6550b1c67f5accfc76c4d` | `01b329f5c54be12d36c6550b1c67f5accfc76c4d` |
| `f326af35240eb7a4d43305ff18a39e68ebcbed6e` | `f326af35240eb7a4d43305ff18a39e68ebcbed6e` |
| `43721b689c045981a6c0e6b1ab3a28a3ccd906a4` | `43721b689c045981a6c0e6b1ab3a28a3ccd906a4` |
| `7dce375da109cc57f1dc18c38b073f8ebbb14f5c` | `7dce375da109cc57f1dc18c38b073f8ebbb14f5c` |
| `712cabd6bf66f74afacbe593a9dcbac31c80801f` | `f8292128563937082b5cf22d94535af013aa7029` |
| `ba65b720808be9721e0fa49067f35ff402be3d77` | `06b3312b770d54f04a8ab3630580842a86fb96e8` |
| `7614ae384a5f5269fd0817fb754ef99adf504050` | `9832c472cf0a1d01147b4469919c2ed96eea4fb1` |
| `3b6329beb0cc1e9342d095bd0e9bacb07e50bf76` | `a87c8593ed0da81bbeaa6e0cd0243e70e4b8146e` |
| `b5260fe0e0040856fd853f4731a750b444f0631b` | `eda7fea5aabbb8470bc381a870f157caa6b90233` |
| `092e51422460603575a5d1007447dfba13c51437` | `d8add7cb6a81422381c88583dcc7246dc403565c` |
| `057ae8153199b572927ce34b84c4eb1c044b6abb` | `94477e229c9c1e529a04a364d8d9bd6497240312` |
| `8e41ea07b672df8eda2b5fa5fb7f24c75af643ea` | `59f8d6e93aa63b8338bf79be530fa4c206a06319` |
| `d9130b88ab55c77475c0b830cfa17d7d5860b6ee` | `c89c84d73ea53cef04400cf864e8c82435cf24d7` |
| `3a81eade1de1eca03a2616116d688fc70b28e198` | `9f19f34173e296d945d903dde004a8b679262136` |
| `d9b54fa6ba7dd911412dd0c9db4afe4ea0d77f11` | `2d3be3e8c003cdeaa21aec11d02663b16ad3f3df` |
| `fb310450d50a4af86a0818a3c66ba9fb6bb72b56` | `a1f315d9e2ab407af32d64b97354b04649e03aca` |
| `4c2ecdcc6959cce18f2e48263bb8de4db3e4178d` | `a97bb789e519ba157c61ebfae7a9c977cfa89945` |
| `fdaa5a8e97b587735aceb5c0914ef7710986b85a` | `94eb375026cf778672f83620133de3343894e5f5` |
| `24455748a121d23051066ff832ad5a4854c7cc05` | `6c593af8f99bce3ef00241cf196f8f31e467e85a` |
| `c474d197e0edbcf0ffe766a888c4b0d699115fcf` | `86d19aef907596d57c2b80e5e8312292ab8cea1f` |

## 剩余边界

GitHub 仓库继续 private。历史个人示例路径、内部说明、原测试地址及远端 Actions 记录尚未清理，见[敏感信息审计](2026-10-27-sensitive-information-audit.md)。仅修正邮箱不代表整个历史已完成公开前隐私清理。

原仓库 reflog、本机备份、其他已有 clone 或 GitHub 旧提交直链 / 运行记录可能仍保存旧对象；本轮结论为远端 main 可达历史的邮箱已统一，不宣称删除所有副本。其他电脑应保存本地修改后重新 clone，或按旧 → 新映射同步，避免 merge / push 把旧历史重新引入。
