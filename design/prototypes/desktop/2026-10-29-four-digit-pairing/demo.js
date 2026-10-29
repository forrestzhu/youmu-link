'use strict';
const dictionaries = {
 zh: {
  title:'四位配对体验',reviewNote:'交互讨论稿 · 所有设备与连接均为演示',scenario:'查看状态',compact:'窄面板',changePort:'模拟端口变化',restart:'重新演示',questCaption:'头显里的游目',questCaptionNote:'原位输入，大按键',desktopCaption:'电脑上的 YM_Link',desktopCaptionNote:'同一次配对',local:'局域网直连',devices:'设备',sameNetwork:'同一局域网',demoNote:'右侧电脑显示的数字就是本次演示配对码。',
  statuses:{discovered:'已发现',searching:'搜索中',handshake:'连接中',pairing:'等待配对',verifying:'正在配对',connected:'已连接',error:'等待配对',expired:'已过期',limited:'请稍后重试',disconnected:'已断开',incompatible:'需要更新'},
  scenes:{pairing:'四位配对',discovered:'发现与品牌入口',searching:'搜索设备',error:'输错后重试',expired:'配对码过期',limited:'频繁尝试',disconnected:'连接中断',incompatible:'另一端需要更新',connected:'连接成功'},
  nearby:'附近的电脑',nearbyHeadset:'附近的头显',computer:'书房 MacBook Pro',headset:'游目 · Quest 3S',chooseTitle:'连接电脑上的 YM_Link',chooseCopy:'在同一 Wi-Fi 下打开电脑上的 YM_Link。选择发现的电脑后，在这里完成配对。',notPaired:'未配对',connect:'连接',searchTitle:'正在搜索附近电脑',searchCopy:'请确认电脑上的 YM_Link 已打开，并连接同一 Wi-Fi。',rescan:'重新搜索',handshakeTitle:'正在连接电脑',handshakeCopy:'保持 YM_Link 在两端打开。',cancel:'取消',pairTitle:'输入配对码',pairCopy:'查看电脑上的 YM_Link，输入它显示的四位数字。',autoSubmitting:code=>`已输入 ${code.split('').join(' ')}，正在配对`,codeLabel:'配对码',pairingCopy:'正在确认这次连接…',delete:'删除上一位',filled:n=>`已输入 ${n} 位，共四位`,waitingTitle:'在头显上输入这四位数字',waitingCopy:'在游目的 YM_Link 面板中输入。',valid:'有效期剩余',waitingOther:'等待头显确认',cancelPair:'取消配对',macReadyTitle:'等待游目头显',macReadyCopy:'在头显中打开游目 → 网络 → YM_Link，选择这台电脑。',macConnectedTitle:'游目已连接',macConnectedCopy:'这台头显已完成配对，可以使用授权的电脑内容。',connectedTitle:'电脑已连接',connectedCopy:'下次连接这台电脑时，无需再次输入配对码。',device:'电脑',connection:'连接状态',paired:'已配对',viewConnection:'查看连接',inputAnnouncement:code=>code?`当前输入 ${code.split('').join(' ')}，${code.length} 位，共四位`:'尚未输入数字，共四位',backList:'返回电脑列表',disconnect:'断开连接',wrongTitle:'配对码不正确',wrongCopy:'配对码不正确，请重新输入。',retry:'重新连接',expiredTitle:'配对码已过期',expiredCopy:'重新连接可获取新的四位码。旧码不再有效。',expiredMac:'本次配对码已失效',limitedTitle:'请稍后再连接',limitedCopy:'尝试次数较多，稍后可重新连接。',retryIn:n=>`${n} 秒后可重试`,disconnectedTitle:'与电脑的连接已断开',disconnectedCopy:'请确认两端仍在同一 Wi-Fi，然后重新连接。',incompatibleTitle:'请更新另一端的应用',incompatibleCopy:'这台电脑的 YM_Link 使用旧版配对方式。请更新电脑上的 YM_Link 与头显里的游目后再连接。',incompatibleMac:'需要更新游目与 YM_Link',understand:'返回列表',macErrorTitle:'本次配对未完成',macErrorCopy:'在头显上重新连接，获取新的配对码。',searchMac:'正在搜索游目头显',readyMac:'等待连接',portUpdate:'端口已更新，仍是同一台头显。',newCode:'生成新配对码',questSummary:'只在当前面板输入，不会弹出独立键盘。',displayOnly:'电脑显示码，头显输入码',entered:'正在等待你输入',connectedFooter:'已配对 · 连接就绪'
 },
 en: {
  title:'Four-digit pairing',reviewNote:'Interactive proposal · all devices and connections are simulated',scenario:'View state',compact:'Narrow panel',changePort:'Change discovery port',restart:'Restart demo',questCaption:'Youmu on Quest',questCaptionNote:'One panel. Larger keys.',desktopCaption:'YM_Link on your computer',desktopCaptionNote:'The same pairing session',local:'Local network',devices:'Devices',sameNetwork:'Same local network',demoNote:'The code on the computer is the code for this demo session.',
  statuses:{discovered:'Discovered',searching:'Searching',handshake:'Connecting',pairing:'Awaiting pairing',verifying:'Pairing',connected:'Connected',error:'Awaiting pairing',expired:'Expired',limited:'Try again later',disconnected:'Disconnected',incompatible:'Update required'},
  scenes:{pairing:'Four-digit pairing',discovered:'Discovery and brand entry',searching:'Searching',error:'Wrong code and retry',expired:'Expired code',limited:'Too many attempts',disconnected:'Connection interrupted',incompatible:'Other app needs an update',connected:'Connected'},
  nearby:'Nearby computer',nearbyHeadset:'Nearby headset',computer:'Study MacBook Pro',headset:'Youmu · Quest 3S',chooseTitle:'Connect with YM_Link',chooseCopy:'Open YM_Link on a computer on the same Wi-Fi. Select the computer, then pair here.',notPaired:'Not paired',connect:'Connect',searchTitle:'Looking for nearby computers',searchCopy:'Open YM_Link on your computer and check that both devices use the same Wi-Fi.',rescan:'Search again',handshakeTitle:'Connecting to your computer',handshakeCopy:'Keep YM_Link open on both devices.',cancel:'Cancel',pairTitle:'Enter the pairing code',pairCopy:'Find the four digits in YM_Link on your computer.',autoSubmitting:code=>`Entered ${code.split('').join(' ')}. Pairing`,codeLabel:'Pairing code',pairingCopy:'Confirming this connection…',delete:'Delete last digit',filled:n=>`${n} of four digits entered`,waitingTitle:'Enter these four digits on Quest',waitingCopy:'Enter them in Youmu’s YM_Link panel.',valid:'Time remaining',waitingOther:'Waiting for Quest',cancelPair:'Cancel pairing',macReadyTitle:'Waiting for Youmu',macReadyCopy:'On Quest, open Youmu → Network → YM_Link, then select this computer.',macConnectedTitle:'Youmu is connected',macConnectedCopy:'This headset is paired and can access the content you have shared.',connectedTitle:'Your computer is connected',connectedCopy:'You won’t need a code the next time you connect to this computer.',device:'Computer',connection:'Connection',paired:'Paired',viewConnection:'View connection',inputAnnouncement:code=>code?`Entered ${code.split('').join(' ')}, ${code.length} of four digits`:'No digits entered, four required',backList:'Back to computers',disconnect:'Disconnect',wrongTitle:'The code did not match',wrongCopy:'Incorrect code. Try again.',retry:'Reconnect',expiredTitle:'This pairing code has expired',expiredCopy:'Reconnect to get a new four-digit code. The previous code no longer works.',expiredMac:'This pairing code is no longer valid',limitedTitle:'Please try again shortly',limitedCopy:'There have been too many attempts. You can reconnect after a short wait.',retryIn:n=>`Try again in ${n}s`,disconnectedTitle:'The connection was interrupted',disconnectedCopy:'Check that both devices still use the same Wi-Fi, then reconnect.',incompatibleTitle:'Update the other app',incompatibleCopy:'This computer uses an older pairing method. Update YM_Link on the computer and Youmu on Quest, then reconnect.',incompatibleMac:'Update Youmu and YM_Link',understand:'Back to computers',macErrorTitle:'Pairing was not completed',macErrorCopy:'Reconnect on Quest to get a new pairing code.',searchMac:'Looking for Youmu',readyMac:'Waiting to connect',portUpdate:'Port updated. This is still the same headset.',newCode:'Get a new code',questSummary:'Enter the code in this panel, without a separate keyboard window.',displayOnly:'Computer shows the code. Quest enters it.',entered:'Waiting for your input',connectedFooter:'Paired · ready to use'
 }
};
const codes=['2568','0427','6810','3094'];
let lang='zh', serial=1, epoch=0, pending=null, attempts=[performance.now()];
let state={phase:'pairing',input:'',code:codes[0],endsAt:performance.now()+300000,port:41431,trusted:false,limitedUntil:0,inputError:false};
const $=s=>document.querySelector(s);
const t=key=>dictionaries[lang][key];
const escapeHtml=value=>String(value).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const icon=(name,cls='')=>`<svg class="${cls}" aria-hidden="true"><use href="#i-${name}"/></svg>`;
const qbutton=(label,action,primary=false,disabled=false)=>`<button class="quest-button ${primary?'primary':''}" type="button" data-action="${action}" data-focus="${action}" ${disabled?'disabled':''}>${escapeHtml(label)}</button>`;
const mbutton=(label,action,primary=false,disabled=false)=>`<button class="mac-button ${primary?'primary':''}" type="button" data-action="${action}" ${disabled?'disabled':''}>${escapeHtml(label)}</button>`;
const status=()=>state.phase==='discovered'&&state.trusted?t('paired'):dictionaries[lang].statuses[state.phase];
const isPair=()=>['pairing','verifying','error'].includes(state.phase);
function stopPending(){epoch++;if(pending!==null)clearTimeout(pending);pending=null;}
function schedule(action,ms){const token=epoch;pending=setTimeout(()=>{pending=null;if(token===epoch)action();},ms);}
function announce(message){$('#announcement').textContent=message;}
function setPhase(phase){state.phase=phase;render();announce(status());}
function admitAttempt(){
 const now=performance.now();
 attempts=attempts.filter(at=>now-at<60000);
 if(now<state.limitedUntil){stopPending();state.input='';setPhase('limited');return false;}
 if(attempts.length>=5){
  state.limitedUntil=attempts[0]+60000;
  stopPending();state.input='';setPhase('limited');return false;
 }
 attempts.push(now);return true;
}
function beginPair(immediate=false){
 if(!admitAttempt())return;
 stopPending();state.input='';state.inputError=false;state.listView=false;
 state.code=codes[serial++%codes.length];state.endsAt=performance.now()+300000;
 state.phase=immediate?'pairing':'handshake';render();
 if(!immediate)schedule(()=>setPhase('pairing'),450);
}
function resumePrompt(){
 if(performance.now()>=state.endsAt){expire();return;}
 if(!admitAttempt())return;
 state.input='';setPhase('pairing');
 if(state.inputError)announce(t('wrongCopy'));
}
function rejectCode(){
 stopPending();state.inputError=true;state.trusted=false;setPhase('error');announce(t('wrongCopy'));
 schedule(resumePrompt,520);
}
function cancel(){stopPending();state.input='';state.inputError=false;setPhase('discovered');}
function addDigit(digit){
 if(state.phase!=='pairing'||state.input.length>=4)return;
 state.inputError=false;state.input+=digit;
 if(state.input.length===4){pair();return;}
 render();announce(t('inputAnnouncement')(state.input));
}
function removeDigit(){if(state.phase!=='pairing'||!state.input)return;state.input=state.input.slice(0,-1);render();announce(t('inputAnnouncement')(state.input));}
function pair(){
 if(state.phase!=='pairing'||state.input.length!==4)return;
 if(performance.now()>=state.endsAt){expire();return;}
 const correct=state.input===state.code, deadline=state.endsAt;
 stopPending();setPhase('verifying');announce(t('autoSubmitting')(state.input));
 schedule(()=>{
  if(performance.now()>=deadline){expire();return;}
  if(correct){state.input='';state.inputError=false;state.trusted=true;setPhase('connected');}else rejectCode();
 },650);
}
function reconnect(){
 if(state.phase==='limited'&&performance.now()<state.limitedUntil)return;
 if(state.trusted&&['disconnected','discovered'].includes(state.phase)){
  stopPending();state.input='';setPhase('handshake');schedule(()=>setPhase('connected'),650);
 }else if(state.phase==='limited'&&performance.now()<state.endsAt){
  stopPending();state.inputError=false;resumePrompt();
 }else beginPair();
}
function expire(){if(!isPair()&&state.phase!=='limited')return;stopPending();state.input='';state.inputError=false;setPhase('expired');}
function reset(){stopPending();serial=0;attempts=[];state={phase:'discovered',input:'',code:codes[0],endsAt:0,port:41431,trusted:false,limitedUntil:0,inputError:false};render();}
function chooseScene(scene){
 stopPending();state.input='';state.inputError=false;state.listView=false;state.trusted=scene==='connected'||scene==='disconnected';state.limitedUntil=0;attempts=[];
 if(scene==='pairing'){beginPair(true);return;}
 if(scene==='error'){beginPair(true);state.input='0000';rejectCode();return;}
 if(scene==='expired')state.code=codes[serial++%codes.length];
 if(scene==='limited')state.limitedUntil=performance.now()+10000;
 state.phase=scene;render();
}
function quest(){
 if(state.phase==='connected'&&state.listView)return `<h3>${t('backList')}</h3><div class="peer-row" data-device-key="computer-demo">${icon('computer')}<div class="peer-info"><h4>${t('computer')}</h4><span class="address">${t('connectedFooter')}</span></div>${qbutton(t('viewConnection'),'view',true)}</div>`;
 switch(state.phase){
  case 'discovered': return `<h3>${t('chooseTitle')}</h3><p>${t('chooseCopy')}</p><div class="peer-row" data-device-key="computer-demo">${icon('computer')}<div class="peer-info"><h4>${state.trusted?t('computer'):t('nearby')}</h4><span class="address">192.0.2.24 · ${state.trusted?t('paired'):t('notPaired')}</span></div>${qbutton(t('connect'),'connect',true)}</div>`;
  case 'searching': return `<h3>${t('searchTitle')}</h3><p>${t('searchCopy')}</p>${qbutton(t('rescan'),'scan',true)}<div class="quiet-notice">${icon('wifi')}<span>${t('sameNetwork')}</span></div>`;
  case 'handshake': return `<h3>${t('handshakeTitle')}</h3><p>${t('handshakeCopy')}</p>${qbutton(t('cancel'),'cancel')}`;
  case 'pairing':case 'verifying':case 'error': {
   const busy=state.phase!=='pairing';
   const slots=Array.from({length:4},(_,i)=>`<span class="code-slot ${state.input[i]?'filled':''} ${!busy&&i===state.input.length?'next':''}" aria-hidden="true">${state.input[i]||''}</span>`).join('');
   const pad=['1','2','3','4','5','6','7','8','9','cancel','0','delete'].map(d=>{
    if(d==='cancel')return `<button type="button" class="cancel-key" data-action="cancel" data-focus="cancel">${t('cancel')}</button>`;
    return `<button type="button" class="${d==='delete'?'delete-key':''}" data-action="${d==='delete'?'delete':'digit'}" ${d==='delete'?'':`data-digit="${d}"`} data-focus="key-${d}" aria-label="${escapeHtml(d==='delete'?t('delete'):d)}" ${busy||(d==='delete'?!state.input:state.input.length===4)?'disabled':''}>${d==='delete'?icon('delete'):d}</button>`;
   }).join('');
   return `<div class="pair-grid"><div class="pair-copy"><h3>${t('pairTitle')}</h3><p>${t('pairCopy')}</p><div class="pair-device">${icon('computer')}<strong>${t('computer')}</strong></div></div><div class="pair-entry"><div class="code-input"><div class="code-slots ${state.inputError?'invalid':''} ${state.phase==='error'?'shake':''}" role="group" aria-label="${t('inputAnnouncement')(state.input)}" aria-invalid="${state.inputError}" ${state.inputError?'aria-describedby="pairing-error"':''}>${slots}</div>${state.inputError?`<p id="pairing-error" class="pair-error">${t('wrongCopy')}</p>`:''}</div><div class="keypad" role="group" aria-label="${t('codeLabel')}">${pad}</div></div></div>`;
  }
  case 'connected': return `<div class="success-mark">${icon('check')}</div><h3>${t('connectedTitle')}</h3><p>${t('connectedCopy')}</p><div class="connection-detail"><div><span>${t('device')}</span><strong>${t('computer')}</strong></div><div><span>${t('connection')}</span><strong>${t('paired')}</strong></div></div><div class="pair-actions">${qbutton(t('backList'),'list',true)}${qbutton(t('disconnect'),'disconnect')}</div>`;
  default:{
   const dict={expired:['expiredTitle','expiredCopy'],limited:['limitedTitle','limitedCopy'],disconnected:['disconnectedTitle','disconnectedCopy'],incompatible:['incompatibleTitle','incompatibleCopy']};
   const [heading,body]=dict[state.phase];
   const limited=state.phase==='limited'&&performance.now()<state.limitedUntil;
   return `<div class="success-mark">${icon(state.phase==='incompatible'?'computer':'alert')}</div><h3>${t(heading)}</h3><p>${t(body)}</p><div class="pair-actions">${state.phase==='incompatible'?'':qbutton(limited?t('retryIn')(Math.ceil((state.limitedUntil-performance.now())/1000)):t('retry'),'retry',true,limited)}${qbutton(t('understand'),'list')}</div>`;
  }
 }
}
function desktop(){
 if(isPair())return `<h3>${t('waitingTitle')}</h3><p>${t('waitingCopy')}</p><div class="code-display" aria-label="${t('codeLabel')}: ${state.code}">${state.code.split('').map(n=>`<span>${n}</span>`).join('')}</div><div class="code-expiry">${t('valid')} <strong data-time></strong></div>${mbutton(t('cancelPair'),'cancel')}<p class="mac-notice">${state.phase==='verifying'?t('pairingCopy'):t('waitingOther')}</p>`;
 if(state.phase==='connected')return `<div class="success-mark">${icon('check')}</div><h3>${t('macConnectedTitle')}</h3><p>${t('macConnectedCopy')}</p>${mbutton(t('disconnect'),'disconnect')}`;
 if(['expired','limited','incompatible'].includes(state.phase))return `<h3>${state.phase==='incompatible'?t('incompatibleMac'):t('macErrorTitle')}</h3><p>${state.phase==='expired'?t('expiredMac'):state.phase==='limited'?t('limitedCopy'):state.phase==='incompatible'?t('incompatibleCopy'):t('macErrorCopy')}</p><div class="code-display" aria-hidden="true">${'<span>—</span>'.repeat(4)}</div><p class="mac-notice">${status()}</p>${state.phase==='expired'?mbutton(t('newCode'),'retry',true):''}`;
 return `<h3>${t('macReadyTitle')}</h3><p>${t('macReadyCopy')}</p><div class="quiet-notice">${icon('wifi')}<span>${t('sameNetwork')}</span></div>${mbutton(t('connect'),'connect',true,state.phase==='searching'||state.phase==='handshake')}`;
}
function render(){
 const previous=document.activeElement?.dataset?.focus;
 const fromQuest=$('#quest-stage').contains(document.activeElement);
 document.documentElement.lang=lang==='zh'?'zh-CN':'en';
 document.title=lang==='zh'?'YM_Link · 四位配对体验':'YM_Link · Four-digit pairing';
 document.querySelectorAll('[data-copy]').forEach(el=>{el.textContent=t(el.dataset.copy);});
 $('#language').textContent=lang==='zh'?'English':'中文';$('#language').setAttribute('aria-label',lang==='zh'?'Switch to English':'切换为中文');
 $('#scenario').innerHTML=Object.entries(t('scenes')).map(([id,label])=>`<option value="${id}" ${id===state.phase?'selected':''}>${label}</option>`).join('');
 if(!Object.hasOwn(t('scenes'),state.phase))$('#scenario').value='pairing';
 $('#demo-note').textContent=t('demoNote');
 $('#quest-header-state').textContent=status();$('#quest-footer-state').textContent=isPair()?'':status();
 $('#quest-stage').innerHTML=quest();$('#desktop-stage').innerHTML=desktop();
 const visible=state.phase!=='searching';$('#device-count').textContent=visible?'1':'0';
 $('#desktop-peer').innerHTML=visible?`<div class="mac-peer" data-device-key="quest-demo">${icon('headset')}<strong>${!state.trusted&&['discovered','searching'].includes(state.phase)?t('nearbyHeadset'):t('headset')}</strong><span>192.0.2.9:${state.port}</span><span>${status()}</span></div>`:'';
 $('#desktop-footer').innerHTML=`<span class="status-dot ${state.phase==='connected'?'connected':''}"></span>${state.phase==='connected'?t('connectedFooter'):status()}`;
 $('#quest-window').classList.toggle('compact',$('#compact').checked);
 if(fromQuest){
  const candidate=previous?document.querySelector(`[data-focus="${previous}"]`):null;
  if(candidate&&!candidate.disabled)candidate.focus({preventScroll:true});
  else $('#quest-stage').focus({preventScroll:true});
 }
 updateTime();
}
function updateTime(){
 const seconds=Math.max(0,Math.ceil((state.endsAt-performance.now())/1000));
 document.querySelectorAll('[data-time]').forEach(el=>{el.textContent=`${String(Math.floor(seconds/60)).padStart(2,'0')}:${String(seconds%60).padStart(2,'0')}`;});
 if(state.phase==='limited'){
  const retry=$('#quest-stage [data-action="retry"]');const remaining=Math.max(0,Math.ceil((state.limitedUntil-performance.now())/1000));
  if(retry){retry.textContent=remaining?t('retryIn')(remaining):t('retry');retry.disabled=remaining>0;}
 }
}
document.addEventListener('click',event=>{
 const button=event.target.closest('button[data-action]');if(!button||button.disabled)return;
 switch(button.dataset.action){
  case 'connect':if(state.trusted)reconnect();else beginPair();break;case 'cancel':cancel();break;case 'digit':addDigit(button.dataset.digit);break;case 'delete':removeDigit();break;case 'retry':reconnect();break;
  case 'list':if(state.phase==='connected'){state.listView=true;render();}else{stopPending();state.input='';setPhase('discovered');}break;
  case 'view':state.listView=false;render();break;
  case 'disconnect':stopPending();state.input='';setPhase('disconnected');break;
  case 'scan':stopPending();setPhase('searching');schedule(()=>setPhase('discovered'),700);break;
 }
});
document.addEventListener('keydown',event=>{
 if(!$('#quest-stage').contains(event.target)||event.altKey||event.ctrlKey||event.metaKey)return;
 if(/^\d$/.test(event.key)){event.preventDefault();if(!event.repeat)addDigit(event.key);}
 else if(event.key==='Backspace'){event.preventDefault();removeDigit();}
 else if(event.key==='Escape'){event.preventDefault();if(isPair()||state.phase==='handshake')cancel();}
});
$('#language').addEventListener('click',()=>{lang=lang==='zh'?'en':'zh';render();});
$('#scenario').addEventListener('change',event=>chooseScene(event.target.value));
$('#compact').addEventListener('change',()=>render());
$('#restart').addEventListener('click',reset);
$('#change-port').addEventListener('click',()=>{state.port+=137;render();announce(t('portUpdate'));});
setInterval(()=>{if(isPair()&&performance.now()>=state.endsAt)expire();else updateTime();},1000);
// Explicit review controls, not a protocol or a network API.
window.pairingDemo={snapshot:()=>({...state,lang}),setScene:chooseScene,expire,reset};
render();
