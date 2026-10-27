// YM_Link demo 状态机：纯前端模拟，两视角共享同一连接状态。
// 状态模型与 docs/plans/2026-10-27-demo-design.md 的映射：
//   link: searching | discovered | pairing | connected | disconnected
//   （设计稿的 entering-code 由 pairing + 头显视角呈现；mirroring / streaming /
//    transferring 是 connected 下的场景态，由 hqScreen / mirroring / transfers 表示）
// 渲染策略：状态转移时整体重渲染；500ms tick 只做定向更新（配对倒计时、
// 镜像统计、传输进度），不重建含输入框的区域。

"use strict";

const HEADSET = { name: "Quest 3S · 游目", platform: "Meta Horizon OS", tag: "headset" };
const COMPUTER = { name: "Forrest 的 MacBook Pro", platform: "macOS", tag: "computer" };

const MONITORS = [
  { id: "main", label: "主显示器", res: "3456 × 2234" },
  { id: "second", label: "DELL U2723", res: "2560 × 1440" },
];

const SHARE_DIRS = [
  { path: "/Users/demo/Movies", note: "影片" },
  { path: "D:\\Media\\Movies", note: "Windows 目录示例" },
];

const FILES = {
  root: [
    { name: "电影", dir: true },
    { name: "纪录片", dir: true },
    { name: "遗忘星球 (2025).mkv", size: "18.4 GB", video: true },
    { name: "城市之光 4K Remux.ts", size: "62.1 GB", video: true },
    { name: "旅行视频 2026.mp4", size: "3.2 GB", video: true },
  ],
  电影: [
    { name: "..", dir: true },
    { name: "沙丘 3 (2027).mkv", size: "41.7 GB", video: true },
    { name: "奥本海默 原盘.mkv", size: "78.9 GB", video: true },
    { name: "字幕包", dir: true },
  ],
  纪录片: [
    { name: "..", dir: true },
    { name: "蓝色星球 II S01E01.mkv", size: "7.8 GB", video: true },
  ],
  字幕包: [
    { name: "..", dir: true },
    { name: "遗忘星球.chs.srt", size: "48 KB", video: false },
    { name: "沙丘 3.chs.srt", size: "52 KB", video: false },
  ],
};

// 平铺目录表的父级映射，用于“返回上级”。
const DIR_PARENT = { 电影: "root", 纪录片: "root", 字幕包: "电影" };

const state = {
  link: "searching", // searching | discovered | pairing | connected | disconnected
  pairingCode: "",
  pairingDeadline: 0,
  pairingExpired: false,
  scanToken: 0,
  monitor: "main",
  mirroring: false,
  mirrorStats: { bitrate: 28, fps: 72, latency: 34 },
  hqScreen: "home", // home | files | mirror | streaming | transfers
  hqDir: "root",
  hqCodeDraft: "",
  hqError: "",
  streamingFile: null,
  transfers: [], // {name, percent, status}
  timer: null,
};

const $ = (id) => document.getElementById(id);
const show = (el, on) => el.classList.toggle("hidden", !on);

function randomCode() {
  return String(Math.floor(100000 + Math.random() * 900000));
}

/* ---------------- 全局状态迁移 ---------------- */

function toSearching() {
  state.link = "searching";
  state.pairingExpired = false;
  state.hqError = "";
  state.scanToken += 1;
  const token = state.scanToken;
  setTimeout(() => {
    if (state.link === "searching" && token === state.scanToken) {
      state.link = "discovered";
      renderAll();
    }
  }, 1600);
  startTicker();
  renderAll();
}

function startPairing() {
  state.link = "pairing";
  state.pairingCode = randomCode();
  state.pairingExpired = false;
  state.pairingDeadline = Date.now() + 300_000; // 规范：5 分钟
  state.hqCodeDraft = "";
  state.hqError = "";
  renderAll();
}

function completePairing() {
  state.link = "connected";
  state.hqScreen = "home";
  state.hqDir = "root";
  state.hqError = "";
  // 断线窗口内手动重新配对的路径也要兑现“恢复后传输继续”。
  state.transfers.forEach((t) => {
    if (t.status === "暂停" && t.percent < 100) t.status = "传输中";
  });
  renderAll();
}

function dropConnection() {
  const wasConnected = state.link === "connected";
  state.link = "disconnected";
  state.mirroring = false;
  state.transfers = state.transfers.map((t) => ({ ...t, status: "暂停" }));
  renderAll();
  if (wasConnected) {
    setTimeout(() => {
      if (state.link === "disconnected") {
        state.link = "connected";
        // 设计稿：恢复后传输继续。
        state.transfers.forEach((t) => {
          if (t.status === "暂停" && t.percent < 100) t.status = "传输中";
        });
        renderAll();
      }
    }, 2400);
  }
}

/* ---------------- 桌面端渲染 ---------------- */

function monitorLabel() {
  const monitor = MONITORS.find((m) => m.id === state.monitor);
  return monitor ? monitor.label : "";
}

function desktopStatusHtml() {
  const dot = (cls, text) => `<span><span class="dot ${cls}"></span>${text}</span>`;
  if (state.link === "connected" && state.mirroring) {
    return (
      dot("ok", `镜像中 · ${monitorLabel()}`) +
      dot("ok", `${state.mirrorStats.fps} fps`) +
      dot("ok", `码率 ${state.mirrorStats.bitrate} Mbps`) +
      dot("ok", `延迟 ${state.mirrorStats.latency} ms`)
    );
  }
  if (state.link === "connected") return dot("ok", "已连接 " + HEADSET.name) + dot("off", "空闲");
  if (state.link === "pairing") return dot("warn", "等待头显确认配对码…");
  if (state.link === "disconnected") return dot("err", "连接已断开，正在重连…");
  if (state.link === "discovered") return dot("warn", "发现设备，等待连接");
  return dot("off", "搜索中…");
}

function desktopEmptyHtml() {
  if (state.link === "searching") {
    return `<div class="spinner"></div>
      <div>正在搜索同一网络中的游目头显…</div>
      <div class="hint">确认头显已戴上并在游目内打开 YM_Link 面板</div>`;
  }
  if (state.link === "disconnected") {
    return `<div>连接已断开，正在重连…</div>
      <div class="hint">重连后传输自动继续，镜像需重新开启；也可以在左侧重新连接</div>`;
  }
  return `<div>已发现设备</div>
    <div class="hint">在左侧设备卡片点「连接」开始配对</div>`;
}

function renderDesktop() {
  const devices = $("desktop-devices");
  const inSession = state.link === "connected" || state.link === "pairing";

  if (state.link === "searching") {
    devices.innerHTML = `<p class="hint">扫描中…</p>`;
  } else if (state.link === "pairing") {
    devices.innerHTML = deviceCard(HEADSET, "busy", "配对中");
  } else if (state.link === "connected") {
    devices.innerHTML = deviceCard(HEADSET, "ok", "已连接") + `
      <button class="btn danger" style="width:100%" onclick="dropConnection()">断开连接</button>`;
  } else {
    devices.innerHTML = deviceCard(HEADSET, "warn", state.link === "disconnected" ? "重连中" : "已发现") + `
      <button class="btn" style="width:100%" onclick="startPairing()">连接</button>`;
  }

  $("desktop-disconnected").innerHTML = desktopEmptyHtml();
  show($("desktop-disconnected"), !inSession);
  show($("desktop-connected"), inSession);
  $("desktop-status").innerHTML = desktopStatusHtml();

  if (inSession) {
    renderMonitors();
    renderSharedDirs();
  }
  show($("mirror-start"), state.link === "connected" && !state.mirroring);
  show($("mirror-stop"), state.mirroring);
}

function deviceCard(device, chipClass, chipText) {
  return `<div class="device">
    <div class="name">${device.name} <span class="chip ${chipClass}">${chipText}</span></div>
    <div class="meta">${device.platform}</div>
  </div>`;
}

function renderMonitors() {
  const grid = $("monitor-grid");
  grid.innerHTML = MONITORS.map((m) => `
    <div class="monitor ${state.monitor === m.id ? "selected" : ""}" onclick="selectMonitor('${m.id}')">
      <div class="screen">${m.res}</div>
      <div class="label">${m.label}</div>
      <div class="res">${m.res}${state.mirroring && state.monitor === m.id ? " · 镜像中" : ""}</div>
    </div>`).join("");
}

function renderSharedDirs() {
  $("shared-dirs").innerHTML = SHARE_DIRS.map((d) => `
    <div class="transfer-item row between">
      <span><b>${d.path}</b> <span class="chip">${d.note}</span></span>
      <button class="btn secondary" onclick="alert('目录管理在 M3 实现；demo 仅展示')">管理</button>
    </div>`).join("");
}

function selectMonitor(id) {
  state.monitor = id;
  renderAll();
}

function startMirror() {
  state.mirroring = true;
  state.mirrorStats = { bitrate: 28, fps: 72, latency: 34 };
  renderAll();
}
function stopMirror() {
  state.mirroring = false;
  renderAll();
}

/* ---------------- 桌面端配对弹窗 ---------------- */

function renderPairingModal() {
  const root = $("modal-root");
  if (state.link !== "pairing") {
    root.innerHTML = "";
    return;
  }
  root.innerHTML = `
    <div class="modal-mask">
      <div class="modal">
        <h3>配对确认</h3>
        <p>${state.pairingExpired
          ? "配对码已过期。请取消后在任一侧重新发起连接，获取新的配对码。"
          : "在头显的 YM_Link 面板输入以下六位码完成配对。"}</p>
        <div class="code">${state.pairingExpired ? "––––––" : state.pairingCode}</div>
        <p id="pair-countdown">${state.pairingExpired ? "" : "有效期剩余 05:00"}</p>
        <div class="row between">
          <button class="btn secondary" onclick="cancelPairing()">取消</button>
          <button class="btn" disabled>${state.pairingExpired ? "配对码已过期" : "等待头显确认…"}</button>
        </div>
      </div>
    </div>`;
}

function updatePairingCountdown() {
  if (state.link !== "pairing" || state.pairingExpired) return;
  const element = $("pair-countdown");
  if (!element) return;
  const secondsLeft = Math.max(0, Math.round((state.pairingDeadline - Date.now()) / 1000));
  const mm = String(Math.floor(secondsLeft / 60)).padStart(2, "0");
  const ss = String(secondsLeft % 60).padStart(2, "0");
  element.textContent = `有效期剩余 ${mm}:${ss}`;
}

function cancelPairing() {
  state.link = "discovered";
  state.hqError = "";
  renderAll();
}

/* ---------------- 头显端渲染 ---------------- */

function hqDeviceRow() {
  if (state.link === "connected") {
    return `<div class="device">
      <div class="name">${COMPUTER.name} <span class="chip ok">已连接</span></div>
      <div class="meta">${COMPUTER.platform}</div>
      <div class="row"><button class="btn" onclick="openHqScreen('mirror')">电脑屏幕</button>
      <button class="btn secondary" onclick="openHqScreen('files')">电脑文件</button></div>
    </div>`;
  }
  if (state.link === "pairing") {
    const errorLine = state.hqError
      ? `<div class="meta" style="color:var(--err)">${state.hqError}</div>`
      : state.pairingExpired
        ? `<div class="meta" style="color:var(--err)">配对码已过期，请取消后重新连接</div>`
        : "";
    return `<div class="device">
      <div class="name">${COMPUTER.name} <span class="chip busy">配对中</span></div>
      <div class="meta">请输入电脑端 YM_Link 显示的六位码</div>
      ${errorLine}
      <div class="code"><input id="hq-code-input" maxlength="6" inputmode="numeric" placeholder="——————" value="${state.hqCodeDraft.replace(/&/g, "&amp;").replace(/"/g, "&quot;")}" oninput="hqTyping(this)"></div>
      <div class="row"><button class="btn" onclick="submitHqCode()">确认</button>
      <button class="btn secondary" onclick="cancelPairing()">取消</button></div>
    </div>`;
  }
  if (state.link === "discovered") {
    return `<div class="device">
      <div class="name">${COMPUTER.name} <span class="chip warn">已发现</span></div>
      <div class="meta">${COMPUTER.platform}</div>
      <button class="btn" onclick="startPairing()">连接</button>
    </div>`;
  }
  if (state.link === "disconnected") {
    return `<div class="device">
      <div class="name">${COMPUTER.name} <span class="chip err">连接断开</span></div>
      <div class="meta">正在重连… 播放与传输已暂停，恢复后自动继续</div>
    </div>`;
  }
  return `<div class="empty"><div class="spinner"></div><div>正在搜索同一网络中的电脑…</div>
    <div class="hint">确认电脑已打开 YM_Link，且两者连接同一 Wi-Fi</div></div>`;
}

function renderHeadset() {
  const area = $("hq-area");
  const controls = $("hq-controls");

  if (state.hqScreen === "streaming") {
    const offline = state.link !== "connected";
    area.innerHTML = `
      <div style="width:82%;aspect-ratio:16/9;border-radius:12px;background:linear-gradient(145deg,#1c2434,#10141d);display:flex;align-items:center;justify-content:center;flex-direction:column;gap:8px">
        <div style="font-size:16px">${state.streamingFile ?? "未在播放"}</div>
        <div class="hint">${offline ? "连接已断开，正在重连… 播放已暂停" : `正在从 ${COMPUTER.name} 流式播放 · 已缓冲 18%`}</div>
      </div>`;
    controls.innerHTML = `
      <button class="btn" onclick="stopStreaming()">停止播放</button>
      <button class="btn secondary" onclick="openHqScreen('files')">返回文件</button>`;
    return;
  }

  if (state.hqScreen === "transfers") {
    const offline = state.link !== "connected";
    area.innerHTML = `<div style="width:88%">
      <h2 style="color:var(--muted);font-size:13px;margin-bottom:10px">传输任务</h2>
      ${offline ? `<div class="transfer-item" style="color:var(--err)">连接已断开，正在重连… 传输已暂停，恢复后自动继续</div>` : ""}
      ${state.transfers.length === 0 ? '<p class="hint">暂无传输任务。去电脑文件里选择“传输到本机”。</p>' : ""}
      ${state.transfers.map((t) => `
        <div class="transfer-item">
          <div class="row between"><span>${t.name} <span class="chip ${t.status === "完成" ? "ok" : "busy"}">${t.status}</span></span>
          <span class="hint">${t.percent}%</span></div>
          <div class="progress"><i style="width:${t.percent}%"></i></div>
        </div>`).join("")}
    </div>`;
    controls.innerHTML = `
      <button class="btn secondary" onclick="openHqScreen('home')">返回</button>
      <span class="hint" style="margin-left:auto">传输支持暂停 / 取消 / 断点续传（M3 实现）</span>`;
    return;
  }

  if (state.hqScreen === "files") {
    const offline = state.link !== "connected";
    const list = FILES[state.hqDir] || [];
    area.innerHTML = `<div style="width:88%">
      <div class="breadcrumb"><a onclick="hqEnter('root')">共享目录</a> / ${state.hqDir === "root" ? "" : state.hqDir}</div>
      ${offline ? `<div class="transfer-item" style="color:var(--err);margin-bottom:10px">连接已断开，正在重连… 恢复后可继续浏览与播放</div>` : ""}
      <table class="files">
        <tr><th>名称</th><th>大小</th><th style="width:190px">操作</th></tr>
        ${list.map((f) => f.dir
          ? `<tr class="dir" onclick="hqEnter('${f.name === ".." ? (DIR_PARENT[state.hqDir] ?? "root") : f.name}')"><td>📁 ${f.name === ".." ? "返回上级" : f.name}</td><td>—</td><td></td></tr>`
          : `<tr><td>${f.video ? "🎬" : "📄"} ${f.name}</td><td class="hint">${f.size}</td><td>
             <button class="btn" ${offline ? "disabled" : ""} onclick="streamFile('${f.name}')">直接播放</button>
             <button class="btn secondary" ${offline ? "disabled" : ""} onclick="transferFile('${f.name}')">传输到本机</button></td></tr>`).join("")}
      </table>
    </div>`;
    controls.innerHTML = `
      <button class="btn secondary" onclick="openHqScreen('home')">返回</button>
      <button class="btn secondary" onclick="openHqScreen('transfers')">传输任务</button>
      <span class="hint" style="margin-left:auto">仅显示电脑端授权目录</span>`;
    return;
  }

  if (state.hqScreen === "mirror") {
    area.innerHTML = `
      <div style="width:82%;aspect-ratio:16/9;border-radius:12px;background:linear-gradient(145deg,#1c2434,#10141d);display:flex;align-items:center;justify-content:center;flex-direction:column;gap:8px">
        <div style="font-size:16px">${COMPUTER.name} · 屏幕镜像</div>
        <div class="hint">${state.link === "connected" ? "等待电脑端开始镜像" : "连接已断开，正在重连…"}</div>
      </div>`;
    controls.innerHTML = `<button class="btn secondary" onclick="openHqScreen('home')">返回</button>`;
    return;
  }

  // home
  const focused = document.activeElement === $("hq-code-input");
  const caret = focused ? $("hq-code-input").selectionStart : null;
  area.innerHTML = `<div style="width:70%">${hqDeviceRow()}</div>`;
  controls.innerHTML = state.link === "connected"
    ? `<button class="btn secondary" onclick="openHqScreen('transfers')">传输任务</button>
       <button class="btn danger" onclick="dropConnection()">断开</button>`
    : `<span class="hint">配对一次后，之后会自动重连</span>`;
  if (focused) {
    const input = $("hq-code-input");
    if (input) {
      input.focus();
      if (caret !== null) input.setSelectionRange(caret, caret);
    }
  }
}

function openHqScreen(screen) {
  state.hqScreen = screen;
  renderAll();
}
function hqEnter(dir) {
  state.hqDir = dir;
  renderAll();
}
function hqTyping(input) {
  state.hqCodeDraft = input.value;
}
function submitHqCode() {
  const draft = state.hqCodeDraft;
  if (state.pairingExpired) {
    state.hqError = "配对码已过期，请取消后重新连接";
    renderAll();
    return;
  }
  if (draft === state.pairingCode) {
    completePairing();
    return;
  }
  state.hqError = draft.length === 6 ? "配对码不正确，请对照电脑端重输" : "请输入完整的六位配对码";
  renderAll();
  const input = $("hq-code-input");
  if (input) {
    input.focus();
    input.select();
  }
}
function streamFile(name) {
  state.streamingFile = name;
  state.hqScreen = "streaming";
  renderAll();
}
function stopStreaming() {
  state.streamingFile = null;
  state.hqScreen = "files";
  renderAll();
}
function transferFile(name) {
  state.transfers.push({ name, percent: 0, status: "传输中" });
  state.hqScreen = "transfers";
  renderAll();
}

/* ---------------- 演示控制 ---------------- */

$("ctl-scan").onclick = () => { resetStreamingState(); toSearching(); };
$("ctl-expire").onclick = () => {
  if (state.link !== "pairing") return;
  state.pairingExpired = true;
  renderAll();
};
$("ctl-drop").onclick = () => {
  if (state.link !== "connected") return;
  dropConnection();
};
$("ctl-reset").onclick = () => { resetStreamingState(); toSearching(); };

function resetStreamingState() {
  state.mirroring = false;
  state.streamingFile = null;
  state.transfers = [];
  state.hqScreen = "home";
  state.hqDir = "root";
  state.hqError = "";
}

function tick() {
  // 定向更新：不整体重渲染，避免打断输入与点击。
  if (state.link === "pairing") {
    if (state.pairingDeadline <= Date.now() && !state.pairingExpired) {
      state.pairingExpired = true;
      renderAll();
    } else {
      updatePairingCountdown();
    }
  }
  if (state.mirroring && state.link === "connected") {
    state.mirrorStats.bitrate = 24 + Math.round(Math.random() * 12);
    state.mirrorStats.latency = 28 + Math.round(Math.random() * 14);
    $("desktop-status").innerHTML = desktopStatusHtml();
  }
  if (state.link === "connected" && state.transfers.some((t) => t.status === "传输中" && t.percent < 100)) {
    state.transfers.forEach((t) => {
      if (t.status === "传输中") {
        t.percent = Math.min(100, t.percent + 2 + Math.round(Math.random() * 4));
        if (t.percent >= 100) t.status = "完成";
      }
    });
    if (state.hqScreen === "transfers") renderHeadset();
  }
}

function startTicker() {
  if (state.timer) return;
  state.timer = setInterval(tick, 500);
}

/* ---------------- 视图切换与启动 ---------------- */

$("tab-desktop").onclick = () => switchView("desktop");
$("tab-headset").onclick = () => switchView("headset");
function switchView(view) {
  show($("view-desktop"), view === "desktop");
  show($("view-headset"), view === "headset");
  $("tab-desktop").classList.toggle("active", view === "desktop");
  $("tab-headset").classList.toggle("active", view === "headset");
}

$("desktop-tab-mirror").onclick = () => switchDesktopTab("mirror");
$("desktop-tab-files").onclick = () => switchDesktopTab("files");
function switchDesktopTab(tab) {
  show($("desktop-mirror"), tab === "mirror");
  show($("desktop-files"), tab === "files");
  $("desktop-tab-mirror").classList.toggle("active", tab === "mirror");
  $("desktop-tab-files").classList.toggle("active", tab === "files");
}

$("mirror-start").onclick = startMirror;
$("mirror-stop").onclick = stopMirror;
$("audio-select").onchange = renderDesktop;

function renderAll() {
  renderDesktop();
  renderHeadset();
  renderPairingModal();
}

toSearching();
