'use strict';
'require view';
'require form';
'require rpc';
'require ui';

var callConfigReload = rpc.declare({
	object: 'luci.pslinkb',
	method: 'config_reload'
});

return view.extend({
	render: function() {
		var m, s, o;

		m = new form.Map('pslinkb');

		s = m.section(form.NamedSection, 'live', 'live', _('Live Configuration'));
		s.anonymous = true;

		o = s.option(form.Value, 'room_id', _('Room ID'), _('Live Room ID'));
		o.datatype = 'uinteger';

		o = s.option(form.Value, 'area_v2', _('Area ID'), _('Default 237 - Single Player - Console Game'));
		o.default = '237';

		o = s.option(form.Value, 'title', _('Title'), _('Leave empty for original title'));

		o = s.option(form.ListValue, 'live_mode', _('Live Mode'), _('Auto - One-Click Start | Manual - Manual Control'));
		o.value('auto', 'Auto');
		o.value('manual', 'Manual');
		o.default = 'auto';

		s = m.section(form.NamedSection, 'auth', 'auth', _('Authentication'));
		s.anonymous = true;

		o = s.option(form.Value, 'cookie', _('Cookie'), _('Format: SESSDATA=xxx; bili_jct=xxx'));
		o.password = true;

		s = m.section(form.NamedSection, 'service', 'service', _('Service'));
		s.anonymous = true;

		o = s.option(form.Flag, 'dns_redirect', _('Auto DNS Redirect'), _('Auto control DNS redirect on PSLinkB start/stop'));
		o.default = '1';
		o.rmempty = false;

		return m.render().then(function(mapNode) {
			return [
				E('h2', { 'name': 'title' }, _('PSLinkB')),
				mapNode
			];
		});
	},

	handleSaveApply: function(ev, mode) {
		document.addEventListener('uci-applied', function () {
			callConfigReload().catch(function(){});
		}, { once: true });
		return this.super('handleSaveApply', [ev, mode]);
	},

	addFooter: function() {
		var t = document.querySelector('h2[name="title"]');
		var menu = document.getElementById('tabmenu');
		if (t && menu && menu.parentNode) menu.parentNode.insertBefore(t, menu);
		return this.super('addFooter', []);
	}
});
