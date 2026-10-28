# Quest候选归并与原生Mac配对回归

- Task: [YML-007](../tasks/archive/YML-007-pairing-discovery-regression.md)
- Commit: `db2495d7d49a4be3dbf43f363c38a97e6681c456`
- Host-OS: macOS26.3.1 arm64，Swift6.3.1 /Xcode26.3
- Result: pass
- App-SHA256: `d3d7951b6f5bd9c9a9bca73e82260c5a5f36d0b3e614e132dce5adfba251b91e`
- Device: Meta Quest3S /Android14 /API34 /incremental4069240019900610

范围：桌面候选 /认证提升回归、当前Mac原生应用构建 /启动，以及Quest真实LAN组件配对 /重连。未验证XR手柄射线、真实系统录屏声音或延迟，不发布Release。

## 失败与修复

用户同一Quest反复配对后Mac积累同IP不同端口的附近设备；core按IP更新，而桌面按IP:port追加且旧端口不单独过期。3项初始red实际得到2/1、4/2、3/2的数量错误。修后anonymous行按IP更新且采用稳定discovery:IP标识，认证成功清理同IP旧anonymous，其他可信身份保留。

独立只读审阅指出连接期间若忽略新端口，会丢失断开后的最新拨号路由；已核验并修复。Peer.address是下一次dial提示，更新不修改Active会话、身份、status或持久TrustStore pin。新增4项通过，包括140次端口轮换、多IP、旧重复收敛、认证清理、信任 /连接状态与断开后的最新路由。复审无剩余桌面发现。

## 本机验证

首次lib全套28 passed /1 failed /1 ignored：授权目录重启等待超时，单项原样复跑pass。首次完整verify的core UDP测试AddrInUse：旧Mac应用占用端口，保留原始失败。

正常退出旧app并设RUST_TEST_THREADS=1后，完整harness verify通过 **37 Python、78 Rust /1 ignored、仓库与rustfmt**。Swift **4/4**，本机app构建、ad-hoc签名 /verify、移位中英资源核对通过；CUA启动新窗口成功。没有改共享根、现有用户信任或录屏权限。ignored证据位于artifacts/yml007/；此处不提交制品 /原始日志。

## 真实Quest到原生Mac

Youmu workspace **20261028.2339 /21**：主SHA `36d936d0c72c182895c69e82cd31f08bd843d565e36ba59e7b4bfcf7a44a0a62`，test SHA `3e026b2481219a7794f2464871dbd5d7653509363d62eeb49a570fe116a62d88`，Android输入d8618a68（完整值由相邻仓库YM-293报告绑定）。受控安装 /双SHA确认后运行现有YoumuLinkNativeDeviceTest。

真实Quest发现并发起Noise，原生Mac窗口实际显示“配对设备Youmu Quest”及六位码；agent通过CUA读取并经本轮私有ADB forward送入组件测试。**1/1组件测试通过**，机器回执paired=true、reconnected=true、share_count=1；覆盖SPAKE2、加密共享查询、镜像能力查询、Known重连与端侧临时信任持久化。组件没有UI输入，不代替Android同面板键盘E2E或XR。

组件正常结束关闭测试socket后，Mac仍弹连接中断提示；这发生在配对 /重连回执通过之后，记录为既有断线提示，不冒充本次配对失败或隐藏该现象。关闭提示后CUA确认列表仅1条已配对Youmu Quest，未因重连增殖。随后仅移除本次临时身份，CUA回读该已配对行消失；没有移除用户既有信任。临时Quest私有目录由测试finally删除，精确ADB forward也已清理。相邻游目ignored artifacts/ym293/native-desktop.log及native-desktop-receipt.json保留完整机器回执。

## CI与限制

实现提交已push main并strict通过。CI40715540013的checks /Windows success，gitleaks40715540024 success；首轮macOS0步骤cancelled，官方annotation明确没有获得hosted runner且arm64容量不足。已对失败job单次重跑，**重跑后三平台checks /Windows /macOS全部success**；同一实现提交db2495d，未改源代码或放宽CI。首轮runner容量取消仍保留，不冒充首轮通过。

真实Android数字键盘 /空间面板输入由游目YM-293的完整E2E检查；用户佩戴位置 /手感反馈另验。实际系统录屏 /声音未运行，不从本轮配对通过推断镜像体验完成。

10-29收尾：游目实现3f7afeef已push main，新0007包（主011b5c0d… /testdb153e25…）同样原生Mac组件1/1通过；测试信任移除后CUA确认设备0。相邻YM-293完整20/20 /162语言也已通过，前两轮E2E发现超时最终核验为删旧toggle连带丢失Compose同步点，补入页 /running同步后通过；没有因此改生产发现协议。当前Mac修复版已恢复运行，用户佩戴 /录屏声音仍未代签。
