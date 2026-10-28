# YML-007：修复Quest重复候选与真实配对回归

Status: verifying
Workflow: behavior
Requires-Device: yes
Tests: ymlink_desktop::runtime::tests，新增匿名候选端口轮换 /可信身份边界 /认证清理回归

## Scope

用户只有一台Quest，反复尝试配对后Mac出现同一IP不同临时端口的多条附近设备；本任务修复桌面状态归并及已认证后的旧候选清理。Android品牌 /输入修复见游目YM-293（docs/tasks/YM-293-link-pairing-brand-input.md）。不改线协议、信任权限、用户共享目录或录屏授权；不发布正式Release。根代理顺序实现，只读代理辅助定位。

## Spec

- 发现v1继续匿名，runtime与core统一按IP维护未认证候选：端口 /nonce变化更新原行，列表始终只有当前候选；不能将IP当密钥身份授予信任。
- 可信设备按device_id管理；匿名公告仅更新未来拨号路由，不覆盖活动连接 /身份 /状态，不删除同IP既有可信身份。
- 成功加密认证后提升真实身份并移除同IP遗留匿名候选；新连接使用最新路由提示，已有128条旧候选也能收敛。
- 首次配对仍使用六位码及300秒期限；用户取消 /错误 /重试保持身份验证与资源释放边界。

## Acceptance

- [x] 旧逻辑的同IP端口轮换重复问题有实际red；修后最新端口与候选数量正确。
- [x] 多IP、可信连接保护、成功认证清理旧匿名项有实际cargo回归。
- [ ] 适用完整verify与macOS app构建 /原生窗口核验通过；实际Quest互联按执行证据记录。
- [ ] 两仓任务互链，记录 /main推送及strict完成。

## Verification

先提取现有事件更新逻辑、cargo新回归red，再修复及定向green；稳定后harness verify（仓库 /Python /rustfmt /cargo），Swift测试与app构建。只读复核实际diff，Mac原生窗口重建核对候选列表；真实Quest→Mac配对与Android输入按实际执行另记。未跑XR /录屏声音保持not-run，不从协议测试推断通过。

## Evidence

开工main干净并fetch /ff-only，旧运行app已用CUA读到原生窗口；此刻设备数0，用户截图中的旧端口行不冒充当前状态。只读定位：core record_peer按IP更新，desktop runtime按IP:port追加，持续同IP心跳又阻止统一过期，是确定缺陷。3项初始cargo red已实际复现（轮换2条代替1条、认证4条代替2条、可信清理3条代替2条）；修后新增4项全部通过。匿名行ID固定discovery:IP，保持UI选择稳定，认证成功清除同IP历史匿名项但保留其他可信身份。

只读复审提出活动连接期间也必须更新未来拨号路由，否则core不再重发同nonce /端口事件时会丢失新端口；已核验并修改，连接 /身份 /状态 /持久pin保持，Peer.address继续仅作显示与下一次dial路由。复审认可，无剩余桌面发现。

首次桌面lib全套28通过 /1失败 /1 ignored：authorized_folders重启等待超时，单项原样重跑通过。首次完整verify因现有原生Mac占用UDP发现端口，core query_targets测试AddrInUse失败；保留日志。正常退出旧app、设置RUST_TEST_THREADS=1后完整verify通过：37 Python、78 Rust /1 ignored、rustfmt及仓库检查。Swift4/4、app构建 /资源 /签名校验通过，已通过CUA启动新原生窗口，未请求录屏。制品SHA d3d7951b6f5bd9c9a9bca73e82260c5a5f36d0b3e614e132dce5adfba251b91e，macOS26.3.1 arm64。真实Quest组件互联待本轮Android新包。

## Next

补重复候选与提升身份的回归并修复，构建本机应用，与Quest新配对输入一起验证；不更改现有共享根或录屏权限。
