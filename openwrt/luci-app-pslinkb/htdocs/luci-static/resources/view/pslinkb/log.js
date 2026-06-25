'use strict';
'require view';
'require rpc';
'require ui';

var callLogRead = rpc.declare({
	object: 'luci.pslinkb',
	method: 'log_read',
	params: [ 'lines' ],
	expect: { text: '' }
});

var callLogClear = rpc.declare({
	object: 'luci.pslinkb',
	method: 'log_clear',
	expect: { ok: false }
});

var STYLE = ''
+ '.pslinkb{display:flex;flex-direction:column;gap:12px;width:100%;box-sizing:border-box}'
+ '.pslinkb *,.pslinkb *::before,.pslinkb *::after{box-sizing:border-box}'
+ '.pslinkb .ps-card{padding:0!important;margin:0!important;border:0!important;overflow:hidden}'
+ '.pslinkb .card-head{display:flex;align-items:center;justify-content:space-between;gap:10px;padding:10px 14px;border-bottom:1px solid rgba(128,128,128,0.12)}'
+ '.pslinkb .card-title{font-size:14px;font-weight:600}'
+ '.pslinkb .card-meta{display:flex;align-items:center;gap:8px;font-size:11px;opacity:0.6;flex-shrink:0}'
+ '.pslinkb .icon-btn{width:28px;height:28px;display:inline-flex;align-items:center;justify-content:center;border:1px solid rgba(128,128,128,0.2);border-radius:0.25rem;background:rgba(128,128,128,0.06);cursor:pointer;padding:0;transition:background 0.15s ease,border-color 0.15s ease,transform 0.1s ease}'
+ '.pslinkb .icon-btn:hover{background:rgba(128,128,128,0.14);border-color:rgba(128,128,128,0.35)}'
+ '.pslinkb .icon-btn:active{transform:scale(0.92)}'
+ '.pslinkb .icon-btn svg{width:15px;height:15px}'
+ '.pslinkb .live-dot{width:7px;height:7px;border-radius:50%;background:#22c55e;flex-shrink:0;animation:pslinkb-pulse 2s ease-in-out infinite}'
+ '@keyframes pslinkb-pulse{0%,100%{opacity:1}50%{opacity:0.35}}'
+ '.pslinkb pre{padding:12px 14px;margin:0;max-height:calc(100vh - 240px);overflow-y:auto;font-size:12px;line-height:1.5;font-family:"JetBrains Mono","Fira Code","Cascadia Code",monospace;tab-size:2}'
+ '@media(max-width:640px){.pslinkb{gap:8px}.pslinkb .card-head{padding:8px 10px;flex-wrap:wrap}.pslinkb pre{padding:8px 10px;font-size:11px;max-height:calc(100vh - 200px)}}';

var TRASH_SVG = '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M3 6h18"/><path d="M8 6V4h8v2"/><path d="M19 6v14H5V6"/><path d="M10 11v6"/><path d="M14 11v6"/></svg>';

return view.extend({
	_refresh: function() {
		var pre = document.getElementById('logContent');
		if (!pre) return;
		callLogRead(80).then(function(text) {
			pre.textContent = text || '';
		}).catch(function(){});
	},

	render: function() {
		var self = this;

		if (!document.getElementById('pslinkb-log-css')) {
			var ss = E('style', { id: 'pslinkb-log-css' });
			ss.textContent = STYLE;
			document.head.appendChild(ss);
		}

		var clearBtn = E('button', {
			'class': 'icon-btn',
			'title': _('Clear Log'),
			'click': function() {
				callLogClear().then(function() { self._refresh(); }).catch(function(){});
			}
		});
		clearBtn.innerHTML = TRASH_SVG;

		var card = E('div', { 'class': 'pslinkb' }, [
			E('div', { 'class': 'cbi-section ps-card' }, [
				E('div', { 'class': 'card-head' }, [
					E('span', { 'class': 'card-title' }, _('Log')),
					E('span', { 'class': 'card-meta' }, [
						E('span', { 'class': 'live-dot' }),
						E('span', {}, _('Auto refresh')),
						clearBtn
					])
				]),
				E('pre', { 'id': 'logContent' }, '')
			])
		]);

		setTimeout(function() {
			var t = document.querySelector('h2[name="title"]');
			var menu = document.getElementById('tabmenu');
			if (t && menu && menu.parentNode) menu.parentNode.insertBefore(t, menu);
		}, 0);

		return [
			E('h2', { 'name': 'title' }, _('PSLinkB')),
			card
		];
	},

	addFooter: function() {
		var self = this;
		this._refresh();
		this._logInterval = setInterval(function() { self._refresh(); }, 1000);
		return E([]);
	}
});
