'use strict';
'require view';
'require rpc';
'require ui';

// ── rpcd ────────────────────────────────

var callStatus = rpc.declare({
	object: 'luci.pslinkb',
	method: 'status_get'
});

var callDnsStatus = rpc.declare({
	object: 'luci.pslinkb',
	method: 'dns_status_get'
});

var callAppInfo = rpc.declare({
	object: 'luci.pslinkb',
	method: 'app_info'
});

var callInstallPackage = rpc.declare({
	object: 'luci.pslinkb',
	method: 'install_package',
	params: ['data']
});

var callCheckUpdates = rpc.declare({
	object: 'luci.pslinkb',
	method: 'check_updates'
});
var callSvcStart = rpc.declare({
	object: 'luci.pslinkb',
	method: 'svc_start'
});

var callSvcStop = rpc.declare({
	object: 'luci.pslinkb',
	method: 'svc_stop'
});

var callSvcRestart = rpc.declare({
	object: 'luci.pslinkb',
	method: 'svc_restart'
});

var callDnsToggle = rpc.declare({
	object: 'luci.pslinkb',
	method: 'dns_toggle',
	params: ['val']
});

// ── CSS ─────────────────────────────────

var STYLE = ''
+ '.pslinkb{display:grid;grid-template-columns:1fr 1fr;gap:12px;width:100%;max-width:100%;box-sizing:border-box}'
+ '.pslinkb *,.pslinkb *::before,.pslinkb *::after{box-sizing:border-box}'
+ '.pslinkb .ps-card{padding:14px 16px!important;margin:0!important;border:0!important}'
+ '.pslinkb .ps-subcard{display:flex;flex-direction:column;padding:10px 12px!important;gap:0;margin:0!important;border:0!important}'
+ '.pslinkb .dual-row{display:grid;grid-template-columns:1fr 1fr;gap:10px;border-radius:inherit}'
+ '.pslinkb .status-item{background:rgba(128,128,128,0.06);border:1px solid rgba(128,128,128,0.10);border-radius:inherit;padding:16px 18px;text-align:center}'
+ '.pslinkb .status-item .si-name{font-size:11px;opacity:0.6;margin-bottom:8px;text-transform:uppercase;letter-spacing:0.5px;font-weight:500}'
+ '.pslinkb .status-item .si-state{font-size:13px;font-weight:600;color:#2dce89;display:flex;align-items:center;justify-content:center;gap:6px}'
+ '.pslinkb .status-item .si-state::before{content:"";width:6px;height:6px;border-radius:50%;background:#2dce89;flex-shrink:0}'
+ '.pslinkb .status-item.stopped .si-state{color:#f5365c;opacity:0.8}'
+ '.pslinkb .status-item.stopped .si-state::before{background:#f5365c;opacity:0.8}'
+ '.pslinkb .status-item.waiting .si-state{color:#fb6340}'
+ '.pslinkb .status-item.waiting .si-state::before{background:#fb6340;animation:pslinkb-pulse 1.2s ease-in-out infinite}'
+ '.pslinkb .status-item.checking .si-state{color:#5e72e4}'
+ '.pslinkb .status-item.checking .si-state::before{background:#5e72e4;animation:pslinkb-pulse 0.8s ease-in-out infinite}'
+ '.pslinkb .control-row{display:flex;align-items:center;justify-content:space-between;gap:10px;min-width:0;border-radius:inherit}'
+ '.pslinkb .dns-label{font-size:14px;font-weight:600;display:inline-block;min-width:56px;text-align:right}'
+ '.pslinkb .dns-label-row{display:flex;align-items:center;gap:8px;flex:1;min-width:0;border-radius:inherit}'
+ '.pslinkb .status-pill{display:inline-flex;align-items:center;gap:5px;padding:2px 8px;border-radius:inherit;font-size:11px;font-weight:600;white-space:nowrap;background:rgba(128,128,128,0.10);border:1px solid rgba(128,128,128,0.18)}'
+ '.pslinkb .status-pill::before{content:"";width:6px;height:6px;border-radius:50%;flex-shrink:0;background:rgba(128,128,128,0.5)}'
+ '.pslinkb .status-pill.running{color:#2dce89;border-color:rgba(45,206,137,0.2);background:rgba(45,206,137,0.08)}'
+ '.pslinkb .status-pill.running::before{background:#2dce89}'
+ '.pslinkb .status-pill.stopped{color:#f5365c;border-color:rgba(245,54,92,0.2);background:rgba(245,54,92,0.08)}'
+ '.pslinkb .status-pill.stopped::before{background:#f5365c}'
+ '.pslinkb .control-bar{display:flex;justify-content:flex-end;align-items:center;gap:8px;margin-top:10px;padding-top:10px;border-top:1px solid rgba(128,128,128,0.12)}'
+ '.pslinkb .control-bar .err-msg{margin-right:auto}'
+ '.pslinkb .control-bar.control-bar-left{justify-content:flex-start}'
+ '.pslinkb .card-row{display:flex;align-items:center;justify-content:space-between;margin-bottom:6px;border-radius:inherit;min-width:0}'
+ '.pslinkb .card-title{font-size:14px;font-weight:600;white-space:nowrap}'
+ '.pslinkb .card-meta{display:flex;align-items:center;gap:5px;font-size:11px;opacity:0.5;flex-shrink:0}'
+ '.pslinkb .live-dot{width:6px;height:6px;border-radius:50%;background:#22c55e;flex-shrink:0;animation:pslinkb-pulse 2s ease-in-out infinite}'
+ '@keyframes pslinkb-pulse{0%,100%{opacity:1}50%{opacity:0.35}}'
+ '.pslinkb .badge{display:inline-flex;align-items:center;gap:4px;padding:2px 8px;border-radius:inherit;font-size:12px;font-weight:600;white-space:nowrap}'
+ '.pslinkb .badge-success{color:#2dce89;background:rgba(45,206,137,0.1)}'
+ '.pslinkb .badge-warning{color:#fb6340;background:rgba(251,99,64,0.1)}'
+ '.pslinkb .badge-info{color:#5e72e4;background:rgba(94,114,228,0.1)}'
+ '.pslinkb .badge-error{color:#f5365c;background:rgba(245,54,92,0.1)}'
+ '.pslinkb .badge-muted{color:#8898aa;background:rgba(128,128,128,0.12)}'
+ '.pslinkb .toggle-group{display:flex;align-items:center;gap:6px;flex-shrink:0}'
+ '.pslinkb .toggle-switch{position:relative;display:inline-block;width:44px;height:24px;cursor:pointer;flex-shrink:0}'
+ '.pslinkb .toggle-switch input{position:absolute;opacity:0;width:0;height:0}'
+ '.pslinkb .toggle-slider{position:absolute;top:0;left:0;right:0;bottom:0;background-color:rgba(128,128,128,0.35);transition:background 0.25s ease;border-radius:24px}'
+ '.pslinkb .toggle-slider::before{content:"";position:absolute;height:18px;width:18px;left:3px;top:3px;background:#fff;transition:transform 0.25s cubic-bezier(0.34,1.56,0.64,1);border-radius:50%;box-shadow:0 1px 3px rgba(0,0,0,0.2)}'
+ '.pslinkb .toggle-switch input:checked+.toggle-slider{background-color:#2dce89}'
+ '.pslinkb .toggle-switch input:checked+.toggle-slider::before{transform:translateX(20px)}'
+ '.pslinkb .toggle-switch.processing .toggle-slider{background-color:#fb6340!important;animation:pulse 0.8s ease-in-out infinite}'
+ '.pslinkb .toggle-switch.processing input:checked+.toggle-slider{background-color:#2dce89!important;animation:pulse 0.8s ease-in-out infinite}'
+ '.pslinkb .toggle-switch input:disabled+.toggle-slider{opacity:0.3;cursor:not-allowed}'
+ '.pslinkb .toggle-switch input:disabled+.toggle-slider::before{opacity:0.5}'
+ '@keyframes pulse{0%,100%{opacity:1}50%{opacity:0.5}}'
+ '.pslinkb .status-pill.ps-processing{background:rgba(128,128,128,0.12);border-color:rgba(128,128,128,0.25);color:#8898aa}'
+ '.pslinkb .status-pill.ps-processing::before{background:#fb6340;animation:pulse 0.8s ease-in-out infinite}'
+ '.pslinkb .icon-btn{display:inline-flex;align-items:center;justify-content:center;width:32px;height:32px;border:1px solid rgba(128,128,128,0.2);border-radius:8px;background:rgba(128,128,128,0.06);cursor:pointer;padding:0;flex-shrink:0;transition:all 0.15s ease}'
+ '.pslinkb .icon-btn:hover{border-color:rgba(128,128,128,0.35)}'
+ '.pslinkb .icon-btn:active{transform:scale(0.95)}'
+ '.pslinkb .icon-btn:disabled{opacity:0.4;cursor:not-allowed}'
+ '.pslinkb .icon-btn:disabled:hover{border-color:rgba(128,128,128,0.2)}'
+ '.pslinkb .icon-btn svg{width:15px;height:15px;opacity:0.5}'
+ '.pslinkb .err-msg{display:none;font-size:12px;color:#f5365c;flex:1;min-width:0}'
+ '.pslinkb .icon-btn:hover svg{opacity:0.8}'
+ '.pslinkb .domain-list{font-size:12px;opacity:0.5;height:32px;display:flex;align-items:center}'
+ '.pslinkb .dns-detail{flex:1;font-size:11px;padding:4px 0 0 0;display:flex;align-items:center;gap:4px}'
+ '.pslinkb .dns-dot{display:inline-block;font-size:12px;font-weight:600;flex-shrink:0;line-height:1}'
+ '.pslinkb .dns-dot.ok{color:#2dce89}'
+ '.pslinkb .dns-dot.fail{color:#f5365c}'
+ '.pslinkb .dns-pill{display:inline-flex;align-items:center;gap:4px;padding:1px 6px;border-radius:4px;font-size:11px;font-weight:500;border:1px solid;font-family:"JetBrains Mono",monospace}'
+ '.pslinkb .dns-pill.ok{color:#2dce89;border-color:rgba(45,206,137,0.25);background:rgba(45,206,137,0.06)}'
+ '.pslinkb .dns-pill.fail{color:#f5365c;border-color:rgba(245,54,92,0.25);background:rgba(245,54,92,0.06)}'
+ '.pslinkb .dns-arrow{opacity:0.3;margin:0 2px}'
+ '.pslinkb .dns-ip{font-family:"JetBrains Mono",monospace;font-size:11px}'
+ '.pslinkb .dns-ip.ok{color:#2dce89}'
+ '.pslinkb .dns-ip.fail{color:#f5365c}'
+ '.pslinkb .dns-ip.muted{color:#8898aa;opacity:0.8}'
+ '.pslinkb-ver{float:right!important;opacity:0.6;white-space:nowrap}'
+ '.pslinkb-ver a{color:inherit;text-decoration:none}'
+ '.pslinkb-ver-new{font-size:0.6em;vertical-align:super;color:#f5365c;font-weight:600;margin-left:1px;opacity:1;animation:pslinkb-pulse 0.8s ease-in-out 3}'
+ '@media(max-width:640px){.pslinkb{grid-template-columns:1fr;gap:8px}.pslinkb .ps-card{padding:10px 12px!important}.pslinkb .card-title{font-size:13px}.pslinkb .badge{font-size:11px;padding:1px 8px}.pslinkb .icon-btn{width:28px;height:28px}.pslinkb .icon-btn svg{width:13px;height:13px}.pslinkb .si-state{font-size:13px}}'
+ '.ps-dialog,.ps-dialog h3,.ps-dialog p{color:var(--cbi-main-fg,#5c6a72)}'
+ '@media(prefers-color-scheme:dark){.ps-dialog,.ps-dialog h3,.ps-dialog p{color:var(--cbi-main-fg,#d3c6aa)}}';

var RESTART_SVG = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M21.5 2v6h-6"/><path d="M2.5 22v-6h6"/><path d="M2 11.5a10 10 0 0 1 18.8-4.3"/><path d="M22 12.5a10 10 0 0 1-18.8 4.2"/></svg>';

function h(tag, attrs, kids) {
	var e = document.createElement(tag);
	for (var k in attrs || {}) {
		if (k === 'className') e.className = attrs[k];
		else if (k === 'innerHTML') e.innerHTML = attrs[k];
		else if (k === 'style') e.style.cssText = attrs[k];
		else if (k === 'checked') e.checked = !!attrs[k];
		else e.setAttribute(k, attrs[k]);
	}
	if (kids) {
		if (typeof kids === 'string' || typeof kids === 'number') e.appendChild(document.createTextNode(kids));
		else if (kids.nodeType) e.appendChild(kids);
		else if (Array.isArray(kids)) {
			for (var i = 0; i < kids.length; i++) {
				var c = kids[i];
				if (c == null) continue;
				if (typeof c === 'string' || typeof c === 'number') e.appendChild(document.createTextNode(c));
				else e.appendChild(c);
			}
		}
	}
	return e;
}

var T = {};

return view.extend({
	load: function() {
		T.RUN = _('Running'); T.STOP = _('Stopped');
		T.STARTING = _('Starting'); T.STOPPING = _('Stopping');
		T.AVL = _('Live stream available'); T.PEND = _('Pending');
		T.CHK = _('Checking'); T.TIMEOUT = _('Stream timeout');
		T.IDLE = _('Idle'); T.NLOGIN = _('Not logged in');
		T.ACTIVE = _('DNS active'); T.INACTIVE = _('DNS inactive');
		T.CHECKING = _('Checking'); T.CLOSING = _('Closing');
		T.STREAMING = _('Streaming'); T.CRASHED = _('Crashed');
		T.RESTARTING = _('Restarting'); T.READY = _('Ready');

		if (!document.getElementById('pslinkb-css')) {
			var ss = h('style', { id: 'pslinkb-css', innerHTML: STYLE });
			document.head.appendChild(ss);
		}

		return Promise.all([
			callStatus().catch(function() { return null; }),
			callDnsStatus().catch(function() { return null; }),
			callAppInfo().catch(function() { return { ver: '', luci_ver: '', latest_ver: '', latest_luci: '', pslinkb_url: '', luci_url: '', pkg_type: '', binary_installed: false }; })
		]).then(function(results) {
			window._pslinkbInit = {
				svc: results[0],
				dns: results[1],
				info: results[2]
			};
		});
	},

	render: function() {
		var d = window._pslinkbInit || {};
		if (d.svc) this._lastRunning = d.svc.running;
		var self = this;

		setTimeout(function() {
			var t = document.querySelector('h2[name="title"]');
			var m = document.getElementById('tabmenu');
			if (t && m && m.parentNode) m.parentNode.insertBefore(t, m);
		}, 0);

		return [
			E('h2', { 'name': 'title' }, _('PSLinkB')),
			self._grid(d)
		];
	},

	addFooter: function() {
		var self = this;
		var d2 = window._pslinkbInit || {};
		var info = d2.info || {};
		var verTxt = info.ver ? 'PSLinkB v' + info.ver : 'PSLinkB N/A';
		var appTxt = 'Luci v' + (info.luci_ver || '?');
		var pslinkb_url = info.pslinkb_url || '#';
		var luci_url = info.luci_url || '#';

		var footer = document.querySelector('footer') || document.createElement('footer');
		if (!footer.parentNode) document.body.appendChild(footer);
		var links = footer.querySelectorAll('a');
		var sep = ' | ';

		if (links.length > 1) {
			// Bootstrap/Argon
			var parent = links[0].parentNode;
			var siblings = parent.querySelectorAll('a');
			var items = [
				{ href: pslinkb_url, text: '\u00A9 2026 Urlynn' },
				{ href: pslinkb_url, text: verTxt },
				{ href: luci_url, text: appTxt }
			];
			for (var i = 0; i < items.length; i++) {
				var a = siblings[i];
				if (!a) {
					var lastA = siblings[siblings.length - 1];
					var ns = lastA.nextSibling;
					if (ns && ns.nodeType === 3 && ns.textContent.trim())
						parent.appendChild(ns.cloneNode(true));
					else if (lastA.previousSibling && lastA.previousSibling.nodeType === 3 && lastA.previousSibling.textContent.trim())
						parent.appendChild(lastA.previousSibling.cloneNode(true));
					a = lastA.cloneNode(true);
					parent.appendChild(a);
				}
				a.href = items[i].href;
				a.target = '_blank';
				a.textContent = items[i].text;
			}
		} else if (links.length === 1) {
			// Alpha
			footer.innerHTML =
				'<a href="' + pslinkb_url + '" target="_blank">&copy; 2026 Urlynn \u00B7 ' + verTxt + ' \u00B7 ' + appTxt + '</a>';
		} else {
			footer.innerHTML =
				'<a href="' + pslinkb_url + '" target="_blank">&copy; 2026 Urlynn</a>' +
				' <span style="opacity:0.3">\u00B7</span> ' +
				'<a href="' + pslinkb_url + '" target="_blank">' + verTxt + '</a>' +
				' <span style="opacity:0.3">\u00B7</span> ' +
				'<a href="' + luci_url + '" target="_blank">' + appTxt + '</a>';
		}

		this._statusInterval = setInterval(function() {
			callStatus().then(function(d) { self._updateSvc(d); }).catch(function() {});
		}, 500);
		this._dnsInterval = setInterval(function() {
			callDnsStatus().then(function(d) { self._updateDns(d); }).catch(function() {});
		}, 500);

		// 本地版本
		var info2 = (window._pslinkbInit || {}).info || {};
		var _inject = function() {
			var m = document.getElementById('tabmenu');
			if (!m) return false;
			var ul = m.querySelector('ul') || m;
			var li = ul.querySelector('li.pslinkb-ver');
			if (!li) {
				li = document.createElement('li');
				li.className = 'pslinkb-ver';
				ul.appendChild(li);
			}
			self._renderVer(li, info2.ver || '', info2.luci_ver || '', '', '', info2);
			// 远端版本
			var cached = sessionStorage.getItem('_pslinkbVer2');
			if (cached) {
				var v = JSON.parse(cached);
				self._renderVer(li, info2.ver || '', info2.luci_ver || '', v.latest_ver || '', v.latest_luci || '', info2);
			} else {
				callCheckUpdates().then(function(r) {
					var data = { latest_ver: (r && r.latest_ver) ? r.latest_ver : '', latest_luci: (r && r.latest_luci) ? r.latest_luci : '' };
					sessionStorage.setItem('_pslinkbVer2', JSON.stringify(data));
					self._renderVer(li, info2.ver || '', info2.luci_ver || '', data.latest_ver, data.latest_luci, info2);
				}).catch(function() {});
			}
			return true;
		};
		if (!_inject()) {
			var obs = new MutationObserver(function() {
				if (_inject()) obs.disconnect();
			});
			obs.observe(document.body, { childList: true, subtree: true });
			setTimeout(function() { obs.disconnect(); }, 5000);
		}

		// 未安装检测
		if (!info2.binary_installed) {
			self._showInstallDialog('pslinkb', info2.ver || '?', true);
		}

		return E([]);
	},

	_renderVer: function(li, ver, appVer, latestVer, latestLuci, info) {
		li.innerHTML = '';
		info = info || {};
		if (ver) {
			var pNewer = latestVer && latestVer !== '';
			var a = document.createElement('a');
			if (pNewer) {
				a.href = '#';
				a.onclick = (function(that, name, v) { return function(e) { e.preventDefault(); that._showInstallDialog('pslinkb', v, false); }; })(this, 'PSLinkB', latestVer);
			} else {
				a.href = info.pslinkb_url || '#';
				a.target = '_blank';
			}
			a.appendChild(document.createTextNode('v' + (pNewer ? latestVer : ver)));
			if (pNewer) {
				var n = document.createElement('span');
				n.className = 'pslinkb-ver-new';
				n.textContent = 'NEW';
				a.appendChild(n);
			}
			li.appendChild(a);
		}
		if (appVer) {
			if (ver) li.appendChild(document.createTextNode(' \u00B7 '));
			var lNewer = latestLuci && latestLuci !== '';
			var b = document.createElement('a');
			if (lNewer) {
				b.href = '#';
				b.onclick = (function(that, name, v) { return function(e) { e.preventDefault(); that._showInstallDialog('luci', v, false); }; })(this, 'Luci', latestLuci);
			} else {
				b.href = info.luci_url || '#';
				b.target = '_blank';
			}
			b.appendChild(document.createTextNode('Luci v' + (lNewer ? latestLuci : appVer)));
			if (lNewer) {
				var n2 = document.createElement('span');
				n2.className = 'pslinkb-ver-new';
				n2.textContent = 'NEW';
				b.appendChild(n2);
			}
			li.appendChild(b);
		}
		var t2 = document.querySelector('h2[name="title"]');
		if (t2) {
			var cs = getComputedStyle(t2);
			li.style.fontFamily = cs.fontFamily;
			li.style.fontWeight = cs.fontWeight;
		}
	},

	_showInstallDialog: function(type, ver, force) {
		var name = (type === 'pslinkb') ? 'PSLinkB' : 'Luci';
		var msg = force
			? _('PSLinkB binary not found. Install now?')
			: name + ' v' + ver + ' ' + _('available, install now?');
		var self = this;
		var modal = document.createElement('div');
		modal.style.cssText = 'position:fixed;top:0;left:0;right:0;bottom:0;background:rgba(0,0,0,0.5);display:flex;flex-direction:column;align-items:center;justify-content:center;gap:12px;z-index:9999';
		modal.innerHTML = '<div class="cbi-section ps-dialog" style="padding:24px;max-width:400px;text-align:center;margin:0">'
			+ '<h3 style="margin:0 0 12px">' + (force ? name + ' ' + _('not installed') : _('update available')) + '</h3>'
			+ '<p style="margin:0 0 20px">' + msg + '</p>'
			+ '<div style="display:flex;gap:12px">'
			+ '<button class="cbi-button cbi-button-positive js-install-btn" style="flex:1">' + _('Install') + '</button>'
			+ '<button class="cbi-button cbi-button-reset js-install-cancel" style="flex:1">' + _('Cancel') + '</button>'
			+ '</div>'
			+ '<p class="js-install-msg" style="display:none;margin:12px 0 0;font-size:13px"></p>'
			+ '<div class="js-install-log-win" style="display:none;margin-top:12px;text-align:left;padding:12px;background:rgba(128,128,128,0.06);border-radius:6px">'
			+ '<div style="display:flex;justify-content:space-between;align-items:center;margin:0 0 8px">'
			+ '<span style="font-size:12px;font-weight:600">' + _('Installation failed') + '</span>'
			+ '<span style="font-size:16px;line-height:1;cursor:pointer;opacity:0.5" onclick="this.closest(\'.js-install-log-win\').style.display=\'none\'">\u00D7</span>'
			+ '</div>'
			+ '<pre style="margin:0;max-height:200px;overflow:auto;font-size:11px;white-space:pre-wrap;word-break:break-all;font-family:monospace"></pre>'
			+ '</div>'
			+ '</div>';
		document.body.appendChild(modal);

		var inner = modal.querySelector('.ps-dialog');
		var bg = window.getComputedStyle(inner).backgroundColor;
		if (bg === 'rgba(0, 0, 0, 0)' || bg === 'transparent') {
			inner.style.backgroundColor = window.matchMedia('(prefers-color-scheme: dark)').matches
				? 'rgba(39,46,51,0.5)' : 'rgba(253,246,227,0.5)';
		}

		var cancelBtn = modal.querySelector('.js-install-cancel');
		var installBtn = modal.querySelector('.js-install-btn');
		var msgEl = modal.querySelector('.js-install-msg');
		var logWin = inner.querySelector('.js-install-log-win');
		var logPre = logWin.querySelector('pre');

		cancelBtn.addEventListener('click', function() { modal.remove(); });
		modal.addEventListener('click', function(e) { if (e.target === modal) modal.remove(); });

		installBtn.addEventListener('click', function() {
			installBtn.disabled = true;
			cancelBtn.disabled = true;
			installBtn.textContent = _('Installing');
			msgEl.style.display = 'block';
			msgEl.textContent = '';
			logWin.style.display = 'none';
			logPre.textContent = '';

			callInstallPackage({ type: type, version: ver }).then(function(res) {
				if (res && res.ok) {
					msgEl.textContent = '\u2713 ' + _('Installation succeeded');
					msgEl.style.display = 'block';
					setTimeout(function() { modal.remove(); location.reload(); }, 800);
				} else {
					installBtn.disabled = false;
					cancelBtn.disabled = false;
					installBtn.textContent = _('Install');
					msgEl.style.display = 'none';
					logPre.textContent = (res && res.log) ? res.log : (res && res.error) ? res.error : '';
					logWin.style.display = 'block';
				}
			}).catch(function() {
				installBtn.disabled = false;
				cancelBtn.disabled = false;
				installBtn.textContent = _('Install');
				msgEl.style.display = 'none';
				logPre.textContent = 'RPC request failed';
				logWin.style.display = 'block';
			});
		});
	},

	_grid: function(d) {
		var self = this;
		return h('div', { className: 'pslinkb' }, [
			self._svcCard(d), self._dnsCard(d), self._loginCard(d), self._liveCard(d)
		]);
	},

	_svcCard: function(d) {
		var svc = (d && d.svc) || {};
		var running = svc.running || false;
		var strTxt, strCls;
		if (svc.mode === 'manual' && running) {
			strTxt = svc.rtmp ? T.READY : T.IDLE;
			strCls = svc.rtmp ? '' : 'stopped';
		} else if (svc.stream_crashed) {
			strTxt = T.CRASHED; strCls = 'stopped';
		} else if (svc.streaming) {
			strTxt = T.STREAMING; strCls = '';
		} else {
			strTxt = T.IDLE; strCls = 'stopped';
		}
		return h('div', { className: 'cbi-section ps-subcard' }, [
			h('div', { className: 'dual-row' }, [
				h('div', { className: 'status-item js-svc' + (running ? '' : ' stopped') }, [
					h('div', { className: 'si-name' }, 'PSLinkB'),
					h('div', { className: 'si-state js-svc-state' }, running ? T.RUN : T.STOP)
				]),
				h('div', { className: 'status-item js-str ' + strCls }, [
					h('div', { className: 'si-name' }, 'Stream'),
					h('div', { className: 'si-state js-str-state' }, strTxt)
				])
			]),
			h('div', { className: 'control-bar' }, [
				h('span', { className: 'err-msg js-err' }),
				this._toggleBtn('svc', running),
				this._restartBtn()
			])
		]);
	},

	_dnsCard: function(d) {
		var dns = (d && d.dns) || {};
		var running = (d && d.svc) ? d.svc.running : false;
		var pillTxt = '', pillCls = '';
		if (dns.checking) {
			pillTxt = dns.enabled ? T.CHECKING : T.CLOSING; pillCls = 'ps-processing';
		} else if (dns.ok) {
			pillTxt = T.ACTIVE; pillCls = 'running';
		} else {
			pillTxt = T.INACTIVE; pillCls = 'stopped';
		}
		return h('div', { className: 'cbi-section ps-subcard' }, [
			h('div', { className: 'control-row' }, [
				h('div', { className: 'dns-label-row' }, [
					h('span', { className: 'dns-label' }, _('DNS Redirect')),
					h('span', { className: 'status-pill js-dns-pill ' + pillCls }, pillTxt)
				]),
				h('div', { className: 'toggle-group' }, [
					this._toggleBtn('dns', dns.enabled, !running)
				])
			]),
			h('div', { className: 'dns-detail js-dns-ip', style: 'min-height:22px', innerHTML: this._dnsIpHtml(dns) }),
			h('div', { className: 'control-bar control-bar-left' }, [
				h('div', { className: 'domain-list' }, 'global-contribute.live-video.net · irc.twitch.tv · live.twitch.tv')
			])
		]);
	},

	_loginCard: function(d) {
		var svc = (d && d.svc) || {};
		var user = svc.user || '';
		return h('div', { className: 'cbi-section ps-card' }, [
			h('div', { className: 'card-row' }, [
				h('span', { className: 'card-title' }, _('Login')),
				h('span', { className: 'badge js-user ' + (user ? 'badge-success' : 'badge-error') }, [h('span', {}, user || T.NLOGIN)])
			])
		]);
	},

	_liveCard: function(d) {
		var svc = (d && d.svc) || {};
		if (svc.mode === 'manual') {
			var url = svc.rtmp || '';
			var cl = url ? 'badge-success' : 'badge-muted';
			var badge = h('span', {
				className: 'badge js-push-url ' + cl,
				style: 'display:block;font-size:11px;font-family:"JetBrains Mono",monospace;cursor:pointer;overflow:hidden;white-space:nowrap;text-overflow:ellipsis',
				title: url ? _('Click to copy') : '',
				'data-url': url
			}, url || T.IDLE);
			if (url) {
				badge.addEventListener('click', function(e) {
					var u = this.getAttribute('data-url');
					if (!u) return;
					var ta = document.createElement('textarea');
					ta.value = u;
					ta.style.position = 'fixed';
					ta.style.opacity = '0';
					document.body.appendChild(ta);
					ta.select();
					document.execCommand('copy');
					document.body.removeChild(ta);
					// 弹窗提示
					var tip = document.createElement('div');
					tip.textContent = '✓ ' + _('Copied to clipboard');
					tip.style.cssText = 'position:fixed;z-index:9999;background:#333;color:#fff;padding:6px 12px;border-radius:6px;font-size:12px;pointer-events:none;transition:opacity 0.3s;opacity:1;left:' + (e.clientX + 12) + 'px;top:' + (e.clientY - 36) + 'px;white-space:nowrap';
					document.body.appendChild(tip);
					setTimeout(function() {
						tip.style.opacity = '0';
						setTimeout(function() { document.body.removeChild(tip); }, 300);
					}, 1000);
				});
			}
			return h('div', { className: 'cbi-section ps-card js-live-card', style: 'overflow:hidden' }, [
				h('div', { className: 'card-row', style: 'gap:4px' }, [
					h('span', { className: 'card-title' }, _('Push URL')),
					badge
				])
			]);
		}
		var str = svc.stream || '';
		var badgeMap = { live: 'badge-success', fake: 'badge-warning', probing: 'badge-info', timeout: 'badge-error', offline: 'badge-error' };
		var textMap = { live: T.AVL, fake: T.PEND, probing: T.CHK, timeout: T.TIMEOUT, offline: T.TIMEOUT };
		var cl = badgeMap[str] || 'badge-muted';
		var txt = textMap[str] || T.IDLE;
		return h('div', { className: 'cbi-section ps-card js-live-card' }, [
			h('div', { className: 'card-row' }, [
				h('span', { className: 'card-title' }, _('Live Status')),
				h('span', { className: 'badge js-stream-badge ' + cl }, [h('span', {}, txt)])
			])
		]);
	},

	_toggleBtn: function(id, checked, disabled) {
		var input = h('input', { type: 'checkbox', 'data-toggle': id, checked: !!checked, title: id === 'svc' ? _('Start / Stop') : _('DNS Redirect') });
		if (disabled) input.disabled = true;
		var label = h('label', { className: 'toggle-switch' + (id === 'dns' ? ' js-dns-switch' : '') }, [
			input,
			h('span', { className: 'toggle-slider' })
		]);
		label.querySelector('input').addEventListener('change', this._handleToggle.bind(this));
		return label;
	},

	_restartBtn: function() {
		var self = this;
		var btn = h('button', {
			className: 'icon-btn js-restart-btn',
			title: _('Restart'),
			innerHTML: RESTART_SVG
		});
		btn.addEventListener('click', function() {
			self._restarting = Date.now();
			self._enterRestarting();
			callSvcRestart().catch(function(){});
		});
		return btn;
	},

	_enterRestarting: function() {
		var svcEl = document.querySelector('.js-svc');
		if (svcEl) { svcEl.classList.remove('stopped'); svcEl.classList.add('waiting'); }
		var svcSt = document.querySelector('.js-svc-state');
		if (svcSt) svcSt.textContent = T.RESTARTING;
		var tg = document.querySelector('[data-toggle="svc"]');
		if (tg) tg.disabled = true;
		var rb = document.querySelector('.js-restart-btn');
		if (rb) rb.disabled = true;
	},

	_handleToggle: function(ev) {
		var inp = ev.target;
		var on = inp.checked;
		var id = inp.getAttribute('data-toggle');

		if (id === 'svc') {
			inp.disabled = true;
			this._svcManual = Date.now();
			this._svcDir = on;
			var svcStEl = document.querySelector('.js-svc-state');
			if (svcStEl) svcStEl.textContent = on ? T.STARTING : T.STOPPING;
			var svcEl = document.querySelector('.js-svc');
			if (svcEl) { svcEl.classList.remove('stopped'); svcEl.classList.add('waiting'); }
			(on ? callSvcStart : callSvcStop)().catch(function() {
				inp.disabled = false;
			});
		} else if (id === 'dns') {
			if (inp.disabled) { inp.checked = !on; return; }
			inp.disabled = true;
			this._dnsTarget = on;
			this._dnsManual = Date.now();
			var pill = document.querySelector('.js-dns-pill');
			if (pill) { pill.textContent = on ? T.CHECKING : T.CLOSING; pill.classList.add('ps-processing'); }
			callDnsToggle(on ? '1' : '0');
		}
	},

	_updateSvc: function(d) {
		this._lastRunning = d.running;

		if (this._restarting) {
			if (Date.now() - this._restarting > 2000 && d.running) {
				this._restarting = null;
				var rSvc = document.querySelector('.js-svc');
				if (rSvc) rSvc.classList.remove('waiting');
				var rTg = document.querySelector('[data-toggle="svc"]'); if (rTg) rTg.disabled = false;
				var rBtn = document.querySelector('.js-restart-btn'); if (rBtn) rBtn.disabled = false;
			} else {
				return;
			}
		}

		var isManual = this._svcManual && Date.now() - this._svcManual < 5000;
		var svcEl = document.querySelector('.js-svc');
		if (svcEl && !isManual) {
			svcEl.classList.remove('waiting');
			if (d.running) svcEl.classList.remove('stopped');
			else svcEl.classList.add('stopped');
		}
		var svcSt = document.querySelector('.js-svc-state');
		if (svcSt && !isManual) svcSt.textContent = d.running ? T.RUN : T.STOP;
		var svcToggle = document.querySelector('[data-toggle="svc"]');
		if (svcToggle) {
			if (!this._svcManual || Date.now() - this._svcManual > 5000) {
				svcToggle.checked = d.running;
				svcToggle.disabled = false;
				svcToggle.disabled = false;
				var svcSw = svcToggle.parentElement;
				if (svcSw) svcSw.classList.remove('processing');
			}
		}

		var strTxt, strCls;
		if (d.mode === 'manual' && d.running) {
			strTxt = d.rtmp ? T.READY : T.IDLE;
			strCls = d.rtmp ? '' : 'stopped';
		} else if (d.stream_crashed) {
			strTxt = T.CRASHED; strCls = 'stopped';
		} else if (d.streaming) {
			strTxt = T.STREAMING; strCls = '';
		} else {
			strTxt = T.IDLE; strCls = 'stopped';
		}
		var strEl = document.querySelector('.js-str');
		if (strEl) { strEl.classList.remove('stopped', 'waiting', 'checking'); if (strCls) strEl.classList.add(strCls); }
		var strSt = document.querySelector('.js-str-state');
		if (strSt) strSt.textContent = strTxt;

		var userEl = document.querySelector('.js-user');
		if (userEl) {
			var u = d.user || '';
			var us = userEl.querySelector('span');
			if (us) us.textContent = u || T.NLOGIN;
			userEl.classList.remove('badge-success', 'badge-error');
			userEl.classList.add(u ? 'badge-success' : 'badge-error');
		}
		var streamEl = document.querySelector('.js-stream-badge');
		if (streamEl) {
			var st = d.stream || '';
			var cl = 'badge-muted', txt = T.IDLE;
			if (st === 'live') { cl = 'badge-success'; txt = T.AVL; }
			else if (st === 'fake') { cl = 'badge-warning'; txt = T.PEND; }
			else if (st === 'probing') { cl = 'badge-info'; txt = T.CHK; }
			else if (st === 'timeout' || st === 'offline') { cl = 'badge-error'; txt = T.TIMEOUT; }
			streamEl.classList.remove('badge-success', 'badge-warning', 'badge-info', 'badge-error', 'badge-muted');
			streamEl.classList.add(cl);
			var ss = streamEl.querySelector('span');
			if (ss) ss.textContent = txt;
		}
		var puEl = document.querySelector('.js-push-url');
		if (puEl) {
			var pu = d.rtmp || '';
			puEl.textContent = pu || T.IDLE;
			puEl.setAttribute('data-url', pu);
			puEl.title = pu ? _('Click to copy') : '';
			puEl.style.cursor = pu ? 'pointer' : '';
			puEl.classList.remove('badge-success', 'badge-muted');
			puEl.classList.add(pu ? 'badge-success' : 'badge-muted');
		}
		var errEl = document.querySelector('.js-err');
		if (errEl) { errEl.style.display = d.error ? 'inline-block' : 'none'; errEl.textContent = d.error || ''; }

		if (d.running && d.qr && sessionStorage.getItem('_pslinkb_from_auth') !== '1') {
			location.href = L.env.scriptname + '/admin/services/pslinkb/auth';
		}
		if (!d.qr) {
			sessionStorage.removeItem('_pslinkb_from_auth');
		} else {
			if (sessionStorage.getItem('_pslinkb_from_auth') !== '1') {
				sessionStorage.setItem('_pslinkb_from_auth', '1');
			}
		}

		var dnsToggle = document.querySelector('[data-toggle="dns"]');
		if (dnsToggle && typeof this._dnsTarget === 'undefined') dnsToggle.disabled = !d.running;
	},

	_dnsIpHtml: function(dns) {
		if (!dns || !dns.target) return '';
		if (dns.ok) return '<span class="dns-dot ok">&#10003;</span> <span class="dns-ip ok">' + dns.target + '</span>';
		if (dns.checking) return '<img src="' + L.resource('icons/loading.svg') + '" style="width:12px;height:12px;margin-right:4px;vertical-align:middle"> <span style="opacity:0.3;margin:0 4px">&#8594;</span><span class="dns-ip muted">' + dns.target + '</span>';
		if (dns.actual) return '<span class="dns-dot fail">&#10007;</span> <span class="dns-ip fail">' + dns.actual + '</span><span style="opacity:0.3;margin:0 4px">&#8594;</span><span class="dns-ip muted">' + dns.target + '</span>';
		return '';
	},

	_updateDns: function(d) {
		if (this._lastRunning !== true) return;
		var pill = document.querySelector('.js-dns-pill');
		var pillManual = this._dnsManual && Date.now() - this._dnsManual < 800;
		if (pill && !pillManual) {
			pill.classList.remove('running', 'stopped', 'ps-processing');
			if (d.checking) { pill.textContent = d.enabled ? T.CHECKING : T.CLOSING; pill.classList.add('ps-processing'); }
			else if (d.enabled && d.ok) { pill.textContent = T.ACTIVE; pill.classList.add('running'); }
			else { pill.textContent = T.INACTIVE; pill.classList.add('stopped'); }
		}
		var sw = document.querySelector('[data-toggle="dns"]');
		if (sw) {
			if (!this._dnsManual || Date.now() - this._dnsManual > 2000) {
				sw.checked = d.enabled;
			}
			var dnsSwWrap = sw.parentElement;
			if (dnsSwWrap && !d.checking) dnsSwWrap.classList.remove('processing');
			if (typeof this._dnsTarget !== 'undefined' && d.enabled === this._dnsTarget && !d.checking) { sw.disabled = false; this._dnsTarget = undefined; }
		}

		var ipEl = document.querySelector('.js-dns-ip');
		if (!ipEl) return;

		var prev = this._dnsPrev || {};
		var curr = { checking: d.checking, enabled: d.enabled, ok: d.ok, target: d.target, actual: d.actual };

		if (d.checking && d.enabled) {
			if (d.target) {
				var midActual = d.actual || prev.actual || '?';
				ipEl.innerHTML = '<img src="' + L.resource('icons/loading.svg') + '" style="width:12px;height:12px;margin-right:4px;vertical-align:middle"> <span class="dns-ip fail">' + midActual + '</span><span style="opacity:0.3;margin:0 4px">&#8594;</span><span class="dns-ip muted">' + d.target + '</span>';
			}
		}
		else if (d.checking && !d.enabled) {
			ipEl.innerHTML = '<img src="' + L.resource('icons/loading.svg') + '" style="width:12px;height:12px;margin-right:4px;vertical-align:middle"> <span style="opacity:0.5;font-size:13px">' + _('Restarting Dnsmasq') + '</span>';
		}
		else {
			if (this._dotTimer) { clearInterval(this._dotTimer); this._dotTimer = null; }
			if (prev.checking && !d.enabled) {
				ipEl.innerHTML = '<span style="opacity:0.5;font-size:13px">' + _('Closed successfully!') + '</span>';
				setTimeout(function() { if (ipEl && ipEl.textContent === _('Closed successfully!')) ipEl.innerHTML = ''; }, 1500);
			} else if (!d.enabled) {
				ipEl.innerHTML = '';
			} else if (d.ok) {
				ipEl.innerHTML = '<span class="dns-dot ok">&#10003;</span> <span class="dns-ip ok">' + d.target + '</span>';
			} else if (d.actual) {
				ipEl.innerHTML = '<span class="dns-dot fail">&#10007;</span> <span class="dns-ip fail">' + d.actual + '</span><span style="opacity:0.3;margin:0 4px">&#8594;</span><span class="dns-ip muted">' + d.target + '</span>';
			}
		}
		this._dnsPrev = curr;
	}
});
