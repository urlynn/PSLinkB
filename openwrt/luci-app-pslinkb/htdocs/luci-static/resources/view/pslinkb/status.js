'use strict';
'require view';
'require rpc';
'require ui';
'require uci';

// ── rpcd ────────────────────────────────

var callInitial = rpc.declare({
	object: 'luci.pslinkb',
	method: 'status_initial'
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

var callSvcStatus = rpc.declare({
	object: 'luci.pslinkb',
	method: 'svc_status'
});

var callDnsToggle = rpc.declare({
	object: 'luci.pslinkb',
	method: 'dns_toggle',
	params: ['val']
});

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
		T.NOTINSTALLED = _('Not installed');

		if (!document.getElementById('pslinkb-css')) {
			var link = document.createElement('link');
			link.id = 'pslinkb-css';
			link.rel = 'stylesheet';
			link.href = L.resource('view/pslinkb/status.css');
			document.head.appendChild(link);
		}

		return Promise.all([
			callInitial().catch(function() { return null; }),
			callAppInfo().catch(function() { return { ver: '', luci_ver: '', latest_ver: '', latest_luci: '', pslinkb_url: '', luci_url: '', pkg_type: '', binary_installed: false }; }),
			uci.load('pslinkb').catch(function() {})
		]).then(function(results) {
			var state = results[0] || {};
			var mode = uci.get('pslinkb', '@live[0]', 'live_mode') || 'auto';
			window._pslinkbData = {
				state: state,
				mode: mode,
				info: results[1]
			};
		});
	},

	render: function() {
		var d = window._pslinkbData || {};
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
		var d = window._pslinkbData || {};
		var info = d.info || {};
		var verTxt = (info.binary_installed !== false && info.ver) ? 'PSLinkB v' + info.ver : 'PSLinkB N/A';
		var appTxt = info.luci_ver ? 'Luci v' + info.luci_ver : 'Luci N/A';
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

		self._updateSvc(d.state || {}, d.mode || 'auto');

		// uhttpd-mod-ubus notify: event=event.trigger, data={type:"pslinkb",data:{key,value}}
		var es = new EventSource('/ubus/subscribe/service');
		es.addEventListener('event.trigger', function(e) {
			try {
				var msg = JSON.parse(e.data);
				if (msg.type !== 'pslinkb') return;
				self._applyPush(msg.data);
			} catch(_) {}
		});

		// 本地版本
		var info = (window._pslinkbData || {}).info || {};
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
			self._renderVer(li, info.ver || '', info.luci_ver || '', '', '', info);
			// 远端版本
			var cached = sessionStorage.getItem('_pslinkbVer2');
			if (cached) {
				var v = JSON.parse(cached);
				self._renderVer(li, info.ver || '', info.luci_ver || '', v.latest_ver || '', v.latest_luci || '', info);
			} else {
				callCheckUpdates().then(function(r) {
					var data = { latest_ver: (r && r.latest_ver) ? r.latest_ver : '', latest_luci: (r && r.latest_luci) ? r.latest_luci : '' };
					sessionStorage.setItem('_pslinkbVer2', JSON.stringify(data));
					self._renderVer(li, info.ver || '', info.luci_ver || '', data.latest_ver, data.latest_luci, info);
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
					setTimeout(function() { sessionStorage.removeItem('_pslinkbVer2'); modal.remove(); location.reload(); }, 800);
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
		var state = (d && d.state) || {};
		var mode = d.mode || 'auto';
		var info = (d && d.info) || {};
		var live = state.live || {};
		var status = live.status || '';
		var psInstalled = info.binary_installed !== false;
		var strInstalled = info.stream_installed !== false;
		var strTxt, strCls;
		if (!strInstalled) { strTxt = T.NOTINSTALLED; strCls = 'notinstalled'; }
		else if (mode === 'manual') {
			strTxt = status ? T.READY : T.IDLE;
			strCls = status ? '' : 'stopped';
		} else if (status === 'live') {
			strTxt = T.STREAMING; strCls = '';
		} else if (status === 'timeout' || status === 'offline') {
			strTxt = T.TIMEOUT; strCls = 'stopped';
		} else if (status === 'fake' || status === 'probing') {
			strTxt = T.CHK; strCls = 'checking';
		} else {
			strTxt = T.IDLE; strCls = 'stopped';
		}
		var psItem = h('div', { className: 'status-item js-svc' + (psInstalled ? '' : ' notinstalled') }, [
			h('div', { className: 'si-name' }, 'PSLinkB'),
			h('div', { className: 'si-state js-svc-state' }, psInstalled ? T.RUN : T.NOTINSTALLED)
		]);
		var strItem = h('div', { className: 'status-item js-str ' + strCls }, [
			h('div', { className: 'si-name' }, 'Stream'),
			h('div', { className: 'si-state js-str-state' }, strTxt)
		]);
		return h('div', { className: 'cbi-section ps-subcard' }, [
			h('div', { className: 'dual-row' }, [ psItem, strItem ]),
			h('div', { className: 'control-bar' }, [
				h('span', { className: 'err-msg js-err' }),
				this._toggleBtn('svc', true),
				this._restartBtn()
			])
		]);
	},

	_dnsCard: function(d) {
		var dns = (d && d.state && d.state.dns) || {};
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
					this._toggleBtn('dns', dns.enabled, false)
				])
			]),
			h('div', { className: 'dns-detail js-dns-ip', style: 'min-height:22px', innerHTML: this._dnsIpHtml(dns) }),
			h('div', { className: 'control-bar control-bar-left' }, [
				h('div', { className: 'domain-list' }, 'global-contribute.live-video.net · irc.twitch.tv · live.twitch.tv')
			])
		]);
	},

	_loginCard: function(d) {
		var state = (d && d.state) || {};
		var user = state.user || '';
		return h('div', { className: 'cbi-section ps-card' }, [
			h('div', { className: 'card-row' }, [
				h('span', { className: 'card-title' }, _('Login')),
				h('span', { className: 'badge js-user ' + (user ? 'badge-success' : 'badge-error') }, [h('span', {}, user || T.NLOGIN)])
			])
		]);
	},

	_liveCard: function(d) {
		var state = (d && d.state) || {};
		var mode = d.mode || 'auto';
		var live = state.live || {};
		var status = live.status || '';
		if (mode === 'manual') {
			var url = status.indexOf('rtmp://') === 0 ? status : '';
			var cl = url ? 'badge-success' : 'badge-muted';
			var badge = h('span', {
				className: 'badge js-push-url ' + cl,
				style: 'display:block;font-size:11px;font-family:"JetBrains Mono",monospace;cursor:' + (url ? 'pointer' : 'default') + ';overflow:hidden;white-space:nowrap;text-overflow:ellipsis',
				title: url ? _('Click to copy') : '',
				'data-url': url
			}, url || T.IDLE);
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
				var tip = document.createElement('div');
				tip.textContent = '✓ ' + _('Copied to clipboard');
				tip.style.cssText = 'position:fixed;z-index:9999;background:#333;color:#fff;padding:6px 12px;border-radius:6px;font-size:12px;pointer-events:none;transition:opacity 0.3s;opacity:1;left:' + (e.clientX + 12) + 'px;top:' + (e.clientY - 36) + 'px;white-space:nowrap';
				document.body.appendChild(tip);
				setTimeout(function() {
					tip.style.opacity = '0';
					setTimeout(function() { document.body.removeChild(tip); }, 300);
				}, 1000);
			});
			return h('div', { className: 'cbi-section ps-card js-live-card', style: 'overflow:hidden' }, [
				h('div', { className: 'card-row', style: 'gap:4px' }, [
					h('span', { className: 'card-title' }, _('Push URL')),
					badge
				])
			]);
		}
		var badgeMap = { live: 'badge-success', fake: 'badge-warning', probing: 'badge-info', timeout: 'badge-error', offline: 'badge-error' };
		var textMap = { live: T.AVL, fake: T.PEND, probing: T.CHK, timeout: T.TIMEOUT, offline: T.TIMEOUT };
		var cl = badgeMap[status] || 'badge-muted';
		var txt = textMap[status] || T.IDLE;
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
		var self = this;
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
			(on ? callSvcStart : callSvcStop)().then(function() {
			//SSE running 到达时由 _applyPush 清空
			inp.disabled = false;
		}).catch(function() {
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

	_updateSvc: function(state, mode) {
		mode = mode || this._mode || 'auto';
		this._mode = mode;

		var live = state.live || {};
		var status = live.status || '';
		var running = state.running === true;

		var info = (window._pslinkbData || {}).info || {};
		var psInstalled = info.binary_installed !== false;
		var strInstalled = info.stream_installed !== false;

		if (this._restarting) {
			if (Date.now() - this._restarting > 2000 && running) {
				this._restarting = null;
				var rSvc = document.querySelector('.js-svc');
				if (rSvc) rSvc.classList.remove('waiting');
				var rTg = document.querySelector('[data-toggle="svc"]'); if (rTg) rTg.disabled = false;
				var rBtn = document.querySelector('.js-restart-btn'); if (rBtn) rBtn.disabled = false;
			} else { return; }
		}

		var isManual = this._svcManual && Date.now() - this._svcManual < 5000;

		if (psInstalled) {
			var svcEl = document.querySelector('.js-svc');
			if (svcEl && !this._svcManual) {
				svcEl.classList.remove('waiting');
				if (running) svcEl.classList.remove('stopped');
				else svcEl.classList.add('stopped');
			}
			var svcSt = document.querySelector('.js-svc-state');
			if (svcSt && !this._svcManual) svcSt.textContent = running ? T.RUN : T.STOP;
		}
		var svcToggle = document.querySelector('[data-toggle="svc"]');
		if (svcToggle) {
			if (!this._svcManual) {
				svcToggle.checked = running;
			}
			svcToggle.disabled = false;
			var svcSw = svcToggle.parentElement;
			if (svcSw) svcSw.classList.remove('processing');
		}

		var strTxt, strCls;
		if (mode === 'manual') {
			var hasUrl = status.indexOf('rtmp://') === 0;
			strTxt = hasUrl ? T.READY : T.IDLE;
			strCls = hasUrl ? '' : 'stopped';
		} else if (status === 'live') {
			strTxt = T.STREAMING; strCls = '';
		} else if (status === 'timeout' || status === 'offline') {
			strTxt = T.TIMEOUT; strCls = 'stopped';
		} else if (status === 'fake' || status === 'probing') {
			strTxt = T.CHK; strCls = 'checking';
		} else {
			strTxt = T.IDLE; strCls = 'stopped';
		}

		if (strInstalled) {
			var strEl = document.querySelector('.js-str');
			if (strEl) { strEl.classList.remove('stopped', 'waiting', 'checking'); if (strCls) strEl.classList.add(strCls); }
			var strSt = document.querySelector('.js-str-state');
			if (strSt) strSt.textContent = strTxt;
		}

		var userEl = document.querySelector('.js-user');
		if (userEl) {
			var u = state.user || '';
			var us = userEl.querySelector('span');
			if (us) us.textContent = u || T.NLOGIN;
			userEl.classList.remove('badge-success', 'badge-error');
			userEl.classList.add(u ? 'badge-success' : 'badge-error');
		}
		var streamEl = document.querySelector('.js-stream-badge');
		if (streamEl) {
			var cl = 'badge-muted', txt = T.IDLE;
			if (status === 'live') { cl = 'badge-success'; txt = T.AVL; }
			else if (status === 'fake') { cl = 'badge-warning'; txt = T.PEND; }
			else if (status === 'probing') { cl = 'badge-info'; txt = T.CHK; }
			else if (status === 'timeout' || status === 'offline') { cl = 'badge-error'; txt = T.TIMEOUT; }
			streamEl.classList.remove('badge-success', 'badge-warning', 'badge-info', 'badge-error', 'badge-muted');
			streamEl.classList.add(cl);
			var ss = streamEl.querySelector('span');
			if (ss) ss.textContent = txt;
		}
		var puEl = document.querySelector('.js-push-url');
		if (puEl && mode === 'manual') {
			var url = status.indexOf('rtmp://') === 0 ? status : '';
			puEl.textContent = url || T.IDLE;
			puEl.setAttribute('data-url', url);
			puEl.title = url ? _('Click to copy') : '';
			puEl.style.cursor = url ? 'pointer' : '';
			puEl.classList.remove('badge-success', 'badge-muted');
			puEl.classList.add(url ? 'badge-success' : 'badge-muted');
		}
		var errEl = document.querySelector('.js-err');
		if (errEl) { errEl.style.display = state.error ? 'inline-block' : 'none'; errEl.textContent = state.error || ''; }

		// QR 跳转检测
		var qr = state.qr || {};
		if (qr.url && qr.status === 'waiting' && sessionStorage.getItem('_pslinkb_from_auth') !== '1') {
			location.href = L.env.scriptname + '/admin/services/pslinkb/auth';
		}
		if (!qr.url) sessionStorage.removeItem('_pslinkb_from_auth');

		var dnsToggle = document.querySelector('[data-toggle="dns"]');
		if (dnsToggle && typeof this._dnsTarget === 'undefined') dnsToggle.disabled = !running;
	},

	_dnsIpHtml: function(dns) {
		if (!dns || !dns.target) return '';
		if (dns.ok) return '<span class="dns-dot ok">&#10003;</span> <span class="dns-ip ok">' + dns.target + '</span>';
		if (dns.checking) return '<img src="' + L.resource('icons/loading.svg') + '" style="width:12px;height:12px;margin-right:4px;vertical-align:middle"> <span style="opacity:0.3;margin:0 4px">&#8594;</span><span class="dns-ip muted">' + dns.target + '</span>';
		if (dns.actual) return '<span class="dns-dot fail">&#10007;</span> <span class="dns-ip fail">' + dns.actual + '</span><span style="opacity:0.3;margin:0 4px">&#8594;</span><span class="dns-ip muted">' + dns.target + '</span>';
		return '';
	},

	_applyPush: function(data) {
		var d = window._pslinkbData || {};
		var state = d.state || {};
		var mode = d.mode || 'auto';

		if (data.key === 'live') {
			state.live = data.value || {};
			this._updateSvc(state, mode);
		} else if (data.key === 'qr') {
			state.qr = data.value || {};
			this._updateSvc(state, mode);
		} else if (data.key === 'user') {
			state.user = data.value || '';
			this._updateSvc(state, mode);
		} else if (data.key === 'error') {
			state.error = data.value || '';
			this._updateSvc(state, mode);
		} else if (data.key === 'running') {
			state.running = data.value;
			this._svcManual = null;
			this._updateSvc(state, mode);
		} else if (data.key === 'dns') {
			state.dns = data.value || {};
			this._updateDns(state.dns);
		}
	},
	_updateDns: function(d) {
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
