# YML-017：Windows 安装版性能实测与前台高 CPU 排查

Status: blocked
Workflow: maintenance
Requires-Device: yes
Tests: not-applicable (安装版实际进程与原生窗口性能验收，仅新增证据 / 文档，不改产品代码)

## Scope

用户已安装 YML-014 交付的 Windows x64 包并启动，按其先前授权检查性能。确认安装版来源、完整启动状态，记录空闲及交互后 CPU / 内存；中途版本被并行任务切换后分列结果。后继开发版前台实测约68%单核CPU，继续按线程栈区分应用与观察工具成本。不改目录授权 / 配对 / 采集权限；长期运行、真实 Quest / 镜像与传输性能另验。

## Acceptance

- [x] 核对安装元数据与二进制，确认单进程及完整启动。
- [x] 低负载 / 高CPU区间、实际 CPU / 内存 / 资源计数和版本切换已记录。
- [ ] 完成同版本 / 同可见性、去除观察客户端干扰的对照；确认高CPU根因与处理。
- [x] 用户确认手动三页 / 刷新完成；独立复核算术、线程栈及结论边界。
- [ ] 证据 / STATUS、仓库 check 与交付门禁完成。

## Verification

读取进程累计 CPU 时间，用 Stopwatch 墙钟测量每段实际时长；单核百分比为 CPU 时间差 / 墙钟差 ×100%，另除以16给出按逻辑处理器归一化的百分比，不保证与任务管理器 CPU utility 瞬时读数一致。每秒重新获取 Process，绑定 PID / StartTime，记录 working set / private bytes / handles / threads / responding。computer-use 原生辅助功能仅确认完整启动及可读页面状态；三页面切换与显示器刷新由用户手工确认，地址展开 / 收起未验收。不在采样区间主动发起界面自动化。仅文档改动运行 harness check，完整自动回归 not-applicable。原始数字采样保留忽略入库的 dist/performance，提交脱敏汇总。后续按用户指示，在实际连接VR后继续，记录头显型号 / 系统、电脑系统及具体负载。

## Evidence

开工 main 干净，fetch / ff-only 无新增，记录基线1654d97。单一安装版 PID8332；Windows11 Pro x64 build26200，Ryzen7 5800X，16逻辑处理器。BUILD-INFO 显示源码5e85961b4f385209b33f99481e5daf986f418970。原生辅助功能可读“正在搜索”、真实3840×2160显示器、0设备、未连接 / 镜像 / 传输，UDP47610已绑定。

computer-use 首次截图失败：Direct3D11CaptureFramePool.CreateFreeThreaded / DXGI_ERROR_DEVICE_REMOVED (0x887A0005)；重新获取窗口并关闭截图后，原生辅助功能读取成功。此为自动化工具截图失败，不归因为 YM_Link 采集缺陷，不改变系统图形 / 安全设置。

安装版 YM_Link.exe / YM_Link.dll / ymlink_desktop.dll / BUILD-INFO.json 的 SHA-256 与 YML-014 交付 CI ZIP 四项完全一致，保留真实来源提交。未记录安装位置或共享目录用户路径。

最初两段30s观察为单核1.48770% / 0.77237%，结束时辅助功能报告窗口已最小化，恢复后页面与初始不同；这组可见性 / 操作未受控，只作参考，不作为纯空闲基线。已通知用户短暂停止操作，恢复安装窗口后另测两段30s；同一PID / 启动时间持续有效。

后续事实、完整数字、进程切换原因、两次managed线程栈、独立复核与无效对照见[性能排查报告](../evidence/2026-10-31-windows-idle-performance.md)。安装版两段约0.87%单核；后继开发版最小化结束时约0.05%，恢复前台后约67.73%。UIA相关栈提示观察成本，但未证实根因；禁止将前两段低读数当完整性能通过。

原临时任务号YML-015与并行README任务碰撞，fetch + ff-only同步其归档及后续YML-016后改为YML-017；没有覆盖其他任务。观察原始文件的早期前缀仍为临时编号，均在忽略目录内。

用户于2026-10-31明确要求先暂停，等后续实际连接VR设备再看CPU指标。已停止后续UI操作、进程采样与根因实验，仅整理记录；本轮未改产品代码。纯文档 `harness check` 与diff检查通过，未把未确认的UIA假设写成结论。

## Next

blocked（用户要求暂停）。解除条件：用户实际连接VR设备，并通知恢复性能检查；在此之前不自动重启采样或操作窗口。恢复后先错开其他聊天的窗口操作，验证观察工具干扰，再按真实连接 / 镜像 / 传输负载测量。当前高CPU根因未确认，不宣称已解决；长期与真实设备性能均待验收。
