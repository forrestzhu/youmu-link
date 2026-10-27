# Apache-2.0 开源准备与 CI 实施计划

目标：[YML-004](../tasks/YML-004-opensource-ci.md)。准备可审查的许可证、历史审计、GitHub 三平台自动测试及原生开发包；保持当前 private，公开与历史改写单独由用户决定。

- [x] 审计所有 refs / 历史 author 与 committer 邮箱、全部历史文本及 Cargo.lock / 原生依赖许可。记录实际差异，不用 mailmap 假装改掉原始邮箱。
- [x] 添加 Apache-2.0 原文、项目 NOTICE、workspace SPDX 与第三方收集说明；当前 demo 个人目录泛化。许可保留上游完整文本、版权与例外条款，不将全部依赖重新标为 Apache。
- [x] stdlib 收集器从锁定 Cargo metadata、实际 Rust sysroot 与 project.assets.json 收集许可到 dist/legal；单测覆盖漏许可、精确版本、notices 与安全路径。原生包包含 legal bundle 和 SHA-256。
- [x] 复用 workflow，固定 action SHA、只读 token、三平台 Rust 与原生测试。受信任 main 构建上传 Windows x64 / macOS arm64 ZIP，保留 7 天；不自动做签名 / 公证、安装器或公开 Release。
- [x] macOS 显式静态链接 Rust .a，移除 modulemap 动态 autolink，检查 otool 不指向 Rust 构建目录的 dylib；只配置不能代替 macOS 实测。
- [ ] 本机全门禁、Windows 实际构建 / 收集 / 打包 / hash 和独立审查；提交推送后回读新 workflow run。账户阻塞时记录 runner not-run，不更改付款或额度。

范围边界：Apache-2.0 允许商用与闭源衍生，保留许可及相关声明，提供贡献者范围的专利授权。仓库可见性、Git 历史作者 / 路径的重写与公开时点未获明确执行指令。开源项目本身与真实 Quest 功能验收、签名分发分别记录，不混为一个完成判据。
