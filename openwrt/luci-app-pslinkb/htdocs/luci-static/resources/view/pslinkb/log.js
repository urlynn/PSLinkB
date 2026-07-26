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
			var link = document.createElement('link');
			link.id = 'pslinkb-log-css';
			link.rel = 'stylesheet';
			link.href = L.resource('view/pslinkb/log.css');
			document.head.appendChild(link);
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
