'use strict';
'require view';
'require rpc';
'require ui';
'require uqr';

var callStatus = rpc.declare({
	object: 'luci.pslinkb',
	method: 'status_get'
});

var callDone = rpc.declare({
	object: 'luci.pslinkb',
	method: 'auth_done'
});

// ── 翻译 ──

var T = {};

	return view.extend({
	load: function() {
		T.SCAN  = _('Scan QR to Login');
		T.SCANNED = _('Scanned, please confirm on your phone');
		T.DONE    = _('Login success! Redirecting to status page');
		T.FACE    = _('Face Verification Required');
		T.FACE_DESC = _('Scan QR to complete face verification');
		T.FACE_DONE  = _('Verification complete! Redirecting to status page');
		T.MOBILE  = _('Or open this page on mobile to auto-launch login');
		T.NO_AUTH = _('No Active Verification');
		T.NO_AUTH_DESC = _('No face or QR verification is currently required.');
		T.REDIRECT = _('Redirecting to status page');

		var self = this;
		return callStatus().then(function(d) {
			self._initQr = d.qr;
			self._isLoggedIn = !!d.user;
		}).catch(function() {
			self._initQr = '';
			self._isLoggedIn = false;
		});
	},

	render: function() {
		var self = this;

		// 无 QR
		if (!this._initQr) {
			setTimeout(function() { location.href = L.env.scriptname + '/admin/services/pslinkb/status'; }, 1000);
			var dots = 0;
			setInterval(function() {
				var el = document.getElementById('redirect-msg');
				if (el) el.textContent = T.REDIRECT + ' .'.repeat(dots);
				dots = (dots + 1) % 4;
			}, 250);
			return [
				E('h2', { 'name': 'title' }, _('PSLinkB')),
				E('div', { 'class': 'pslinkb-auth' }, [
				E('div', { 'class': 'cbi-section', 'style': 'text-align:center;padding:24px' }, [
					E('h2', { 'style': 'font-size:15px;margin:0 0 8px 0' }, T.NO_AUTH),
					E('p', { 'style': 'font-size:13px;margin:0 0 12px 0;opacity:0.6' }, T.NO_AUTH_DESC),
					E('p', { 'id': 'redirect-msg', 'style': 'font-size:12px;margin-top:12px' }, T.REDIRECT)
				])
			])];
		}

		// 移动端
		var ua = navigator.userAgent.toLowerCase();
		if (ua.indexOf('iphone') >= 0 || ua.indexOf('ipad') >= 0 || ua.indexOf('android') >= 0 || ua.indexOf('mobile') >= 0) {
			location.href = this._initQr;
			return [
				E('h2', { 'name': 'title' }, _('PSLinkB')),
				E('div', {}, _('Redirecting...'))
			];
		}

		var title = this._isLoggedIn ? T.FACE : T.SCAN;
		var desc  = this._isLoggedIn ? T.FACE_DESC : '';

		return [
			E('h2', { 'name': 'title' }, _('PSLinkB')),
			E('div', { 'class': 'pslinkb-auth' }, [
			E('div', { 'class': 'cbi-section', 'style': 'text-align:center;padding:24px' }, [
				E('h2', { 'id': 'auth-title', 'style': 'font-size:15px;margin:0 0 16px 0' }, title),
				E('p', { 'id': 'auth-desc', 'style': 'font-size:13px;margin:0 0 12px 0;opacity:0.6' }, desc),
				E('div', { 'id': 'qrcode', 'style': 'display:inline-block;margin:8px 0' }),
				E('p', { 'id': 'msg', 'style': 'font-size:12px;margin-top:12px' }, T.MOBILE)
			])
		])];
	},

	addFooter: function() {
		var self = this;

		var t = document.querySelector('h2[name="title"]');
		var menu = document.getElementById('tabmenu');
		if (t && menu && menu.parentNode) menu.parentNode.insertBefore(t, menu);

		sessionStorage.setItem('_pslinkb_from_auth', '1');
		var statusUrl = L.env.scriptname + '/admin/services/pslinkb/status';
		var curQr = this._initQr;
		var box = document.getElementById('qrcode');
		var titleEl = document.getElementById('auth-title');
		var isLoggedIn = this._isLoggedIn;

		function draw(url) {
			box.innerHTML = uqr.renderSVG(url, { pixelSize: 8 });
			curQr = url;
		}

		if (curQr) draw(curQr);

		(function poll(){
			callStatus().then(function(d) {
				var st = d.qr_status || '';
				if (st === 'scanned') {
					titleEl.textContent = T.SCANNED;
					setTimeout(poll, 200);
					return;
				}
				if (st === 'done') {
					callDone();
					titleEl.textContent = isLoggedIn ? T.FACE_DONE : T.DONE;
					setTimeout(function() { location.href = statusUrl; }, 1000);
					return;
				}
				if (d.qr && d.qr !== curQr) draw(d.qr);
				setTimeout(poll, 500);
			}).catch(function() { setTimeout(poll, 500); });
		})();

		return E([]);
	}
});
