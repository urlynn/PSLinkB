'use strict';
'require view';
'require ui';

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
+ '.pslinkb .card-row{display:flex;align-items:center;justify-content:space-between;margin-bottom:6px;border-radius:inherit}'
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
+ '@media(max-width:640px){.pslinkb{grid-template-columns:1fr;gap:8px}.pslinkb .ps-card{padding:10px 12px!important}.pslinkb .card-title{font-size:13px}.pslinkb .badge{font-size:11px;padding:1px 8px}.pslinkb .icon-btn{width:28px;height:28px}.pslinkb .icon-btn svg{width:13px;height:13px}.pslinkb .si-state{font-size:13px}}';

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

var T = {}; // 翻译 key

// ──────────────────────────────────────────────────
	return view.extend({
	load: function() {
		// 缓存翻译
		T.RUN = _('Running'); T.STOP = _('Stopped');
		T.AVL = _('Live stream available'); T.PEND = _('Pending');
		T.CHK = _('Checking'); T.TIMEOUT = _('Stream timeout');
		T.IDLE = _('Idle'); T.NLOGIN = _('Not logged in');
		T.ACTIVE = _('DNS active'); T.INACTIVE = _('DNS inactive');
		T.CHECKING = _('Checking…'); T.CLOSING = _('Closing…');
		T.STREAMING = _('Streaming'); T.CRASHED = _('Crashed');

		var base = L.env.scriptname + '/admin/services/pslinkb';
		this.URLS = {
			status:   base + '/status-json',
			dns:      base + '/dns-status',
			dnsToggle:base + '/dns-toggle',
			start:    base + '/ctl_start',
			stop:     base + '/ctl_stop',
			restart:  base + '/ctl_restart',
		};

		// 注入 CSS
		if (!document.getElementById('pslinkb-css')) {
			var ss = h('style', { id: 'pslinkb-css', innerHTML: STYLE });
			document.head.appendChild(ss);
		}

	return Promise.all([
			fetch(this.URLS.status + '?_=' + Date.now()).then(function(r) { return r.json(); }),
			fetch(this.URLS.dns    + '?_=' + Date.now()).then(function(r) { return r.json(); })
		]).then(function(results) {
			// View 框架 load/render 不同实例，用 window 传递
			window._pslinkbInit = { svc: results[0], dns: results[1] };
		}).catch(function() {
			window._pslinkbInit = {};
		});
	},

	render: function() {
		var d = window._pslinkbInit || {};
		if (d.svc) this._lastRunning = d.svc.running;
		var self = this;
		// 延迟移动标题到 tabmenu 上方（等 DOM 完全插入）
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
		this._statusInterval = setInterval(function() {
			fetch(self.URLS.status + '?_=' + Date.now())
				.then(function(r) { return r.json(); })
				.then(function(d) { self._updateSvc(d); })
				.catch(function() {});
		}, 500);
		this._dnsInterval = setInterval(function() {
			fetch(self.URLS.dns + '?_=' + Date.now())
				.then(function(r) { return r.json(); })
				.then(function(d) { self._updateDns(d); })
				.catch(function() {});
		}, 500);
		return E([]);
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
		// Stream 卡片：pslinkb-stream 二进制状态
		var strTxt, strCls;
		if (svc.stream_crashed) { strTxt = T.CRASHED; strCls = 'stopped'; }
		else if (svc.streaming)   { strTxt = T.STREAMING; strCls = ''; }
		else                     { strTxt = T.IDLE; strCls = 'stopped'; }
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
				h('span', { className: 'badge js-user ' + (user ? 'badge-success' : 'badge-error') }, user || T.NLOGIN)
			])
		]);
	},

	_liveCard: function(d) {
		var svc = (d && d.svc) || {};
		var str = svc.stream || '';
		var badgeMap = { live: 'badge-success', fake: 'badge-warning', probing: 'badge-info', timeout: 'badge-error', offline: 'badge-error' };
		var textMap = { live: T.AVL, fake: T.PEND, probing: T.CHK, timeout: T.TIMEOUT, offline: T.TIMEOUT };
		var cl = badgeMap[str] || 'badge-muted';
		var txt = textMap[str] || T.IDLE;
		return h('div', { className: 'cbi-section ps-card' }, [
			h('div', { className: 'card-row' }, [
				h('span', { className: 'card-title' }, _('Live Status')),
				h('span', { className: 'badge js-stream-badge ' + cl }, txt)
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
			className: 'icon-btn',
			title: _('Restart'),
			innerHTML: RESTART_SVG
		});
		btn.addEventListener('click', function() {
			fetch(self.URLS.restart, {
				method: 'POST',
				headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
				body: 'token=' + L.env.token
			});
		});
		return btn;
	},

	_handleToggle: function(ev) {
		var inp = ev.target;
		var on = inp.checked;
		var id = inp.getAttribute('data-toggle');

		if (id === 'svc') {
			var sw = inp.parentElement;
			if (sw) sw.classList.add('processing');
			// 标记手动操作中，5s 内不响应 poll
			this._svcManual = Date.now();
			fetch(on ? this.URLS.start : this.URLS.stop, {
				method: 'POST',
				headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
				body: 'token=' + L.env.token
			}).catch(function() { if (sw) sw.classList.remove('processing'); });
		} else if (id === 'dns') {
			if (inp.disabled) { inp.checked = !on; return; }  // 服务未运行，忽略操作
			this._dnsManual = Date.now();
			var dnsSw = inp.parentElement;
			if (dnsSw) dnsSw.classList.add('processing');
			var pill = document.querySelector('.js-dns-pill');
			if (pill) { pill.textContent = on ? T.CHECKING : T.CLOSING; pill.classList.add('ps-processing'); }
			fetch(this.URLS.dnsToggle, {
				method: 'POST',
				headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
				body: 'val=' + (on ? '1' : '0')
			});
		}
	},


	_updateSvc: function(d) {
		this._lastRunning = d.running;
		var isManual = this._svcManual && Date.now() - this._svcManual < 5000;
		var svcEl = document.querySelector('.js-svc');
		if (svcEl && !isManual) {
			if (d.running) svcEl.classList.remove('stopped');
			else svcEl.classList.add('stopped');
		}
		var svcSt = document.querySelector('.js-svc-state');
		if (svcSt && !isManual) svcSt.textContent = d.running ? T.RUN : T.STOP;
		var svcToggle = document.querySelector('[data-toggle="svc"]');
		if (svcToggle) {
			// 手动操作后 5s 内不覆盖 checkbox
			if (!this._svcManual || Date.now() - this._svcManual > 5000) {
				svcToggle.checked = d.running;
				var svcSw = svcToggle.parentElement;
				if (svcSw) svcSw.classList.remove('processing');
			}
		}

		// Stream 卡片：pslinkb-stream 二进制状态
		var strTxt, strCls;
		if (d.stream_crashed) { strTxt = T.CRASHED; strCls = 'stopped'; }
		else if (d.streaming)  { strTxt = T.STREAMING; strCls = ''; }
		else                  { strTxt = T.IDLE; strCls = 'stopped'; }
		var strEl = document.querySelector('.js-str');
		if (strEl) { strEl.classList.remove('stopped', 'waiting', 'checking'); if (strCls) strEl.classList.add(strCls); }
		var strSt = document.querySelector('.js-str-state');
		if (strSt) strSt.textContent = strTxt;

		var userEl = document.querySelector('.js-user');
		if (userEl) {
			var u = d.user || '';
			userEl.textContent = u || T.NLOGIN;
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
			streamEl.classList.add(cl); streamEl.textContent = txt;
		}
		var errEl = document.querySelector('.js-err');
		if (errEl) { errEl.style.display = d.error ? 'inline-block' : 'none'; errEl.textContent = d.error || ''; }

		// 未登录时跳转认证页
		if (d.running && d.qr && sessionStorage.getItem('_pslinkb_from_auth') !== '1') {
			location.href = L.env.scriptname + '/admin/services/pslinkb/auth';
		}
		if (!d.qr) {
			sessionStorage.removeItem('_pslinkb_from_auth');
		} else {
			// 标记已跳转，防止重复跳转
			if (sessionStorage.getItem('_pslinkb_from_auth') !== '1') {
				sessionStorage.setItem('_pslinkb_from_auth', '1');
			}
		}

		// DNS 滑块：服务未运行时禁用
		var dnsToggle = document.querySelector('[data-toggle="dns"]');
		if (dnsToggle) dnsToggle.disabled = !d.running;
	},

	_dnsIpHtml: function(dns) {
		if (!dns || !dns.target) return '';
		if (dns.ok) return '<span class="dns-dot ok">&#10003;</span> <span class="dns-ip ok">' + dns.target + '</span>';
		if (dns.checking) return '<img src="' + L.resource('icons/loading.svg') + '" style="width:12px;height:12px;margin-right:4px;vertical-align:middle"> <span style="opacity:0.3;margin:0 4px">&#8594;</span><span class="dns-ip muted">' + dns.target + '</span>';
		if (dns.actual) return '<span class="dns-dot fail">&#10007;</span> <span class="dns-ip fail">' + dns.actual + '</span><span style="opacity:0.3;margin:0 4px">&#8594;</span><span class="dns-ip muted">' + dns.target + '</span>';
		return '';
	},

	_updateDns: function(d) {
		// 仅在确认服务运行后才更新 DNS pill，避免初始闪烁
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
		}

		var ipEl = document.querySelector('.js-dns-ip');
		if (!ipEl) return;

		// 状态机：基于新旧状态 diff 决定 IP 行显示
		var prev = this._dnsPrev || {};
		var curr = { checking: d.checking, enabled: d.enabled, ok: d.ok, target: d.target, actual: d.actual };

		// 开启检测中 — 显示转圈 + 旧IP → 目标IP
		if (d.checking && d.enabled) {
			if (d.target) {
				var midActual = d.actual || prev.actual || '?';
				ipEl.innerHTML = '<img src="' + L.resource('icons/loading.svg') + '" style="width:12px;height:12px;margin-right:4px;vertical-align:middle"> <span class="dns-ip fail">' + midActual + '</span><span style="opacity:0.3;margin:0 4px">&#8594;</span><span class="dns-ip muted">' + d.target + '</span>';
			}
		}
		// 关闭中
		else if (d.checking && !d.enabled) {
			ipEl.innerHTML = '<img src="' + L.resource('icons/loading.svg') + '" style="width:12px;height:12px;margin-right:4px;vertical-align:middle"> <span style="opacity:0.5;font-size:13px">' + _('Restarting Dnsmasq') + '</span>';
		}
		// 操作完成
		else {
			if (this._dotTimer) { clearInterval(this._dotTimer); this._dotTimer = null; }
			// 刚关闭完成（从 checking 转换过来）
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
