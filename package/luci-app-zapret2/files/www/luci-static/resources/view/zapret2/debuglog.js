'use strict';
'require baseclass';
'require form';
'require fs';
'require ui';
'require poll';

var _pollFn = null;

function escapeCssSelector(str) {
	return str.replace(/["\\]/g, '\\$&');
}

var ALLOWED_LOG_DIRS = ['/tmp/zapret2/', '/opt/zapret2/ipset/'];

function isAllowedLogPath(path) {
	if (!path || typeof path !== 'string') return false;
	if (path.indexOf('..') !== -1 || path.indexOf('\0') !== -1) return false;
	var normalized = path.replace(/\/+/g, '/');
	if (!normalized.endsWith('.log')) return false;
	for (var i = 0; i < ALLOWED_LOG_DIRS.length; i++) {
		if (normalized.startsWith(ALLOWED_LOG_DIRS[i])) return true;
	}
	return false;
}

function getLineCount(str) {
	return str ? str.split('\n').length : 0;
}

function formatTitle(cfgname) {
	if (cfgname === 'main') return _('Main');
	if (cfgname === 'autohostlist') return _('Autohostlist');

	var name = cfgname.replace(/^\d+-/, '');
	name = name.replace(/[_-]/g, ' ');
	name = name.replace(/\b\w/g, function(c) { return c.toUpperCase(); });
	return name;
}

function retrieveLogs() {
	return Promise.all([
		fs.exec('/usr/bin/find', ['/tmp/zapret2', '-maxdepth', '1', '-type', 'f', '-name', '*.log']),
		L.resolveDefault(fs.stat('/opt/zapret2/ipset/zapret_hosts_auto_debug.log'), null)
	]).then(function(results) {
		var findResult = results[0] || {};
		var autoStat = results[1];
		return Promise.all([
			findResult,
			autoStat ? L.resolveDefault(fs.read_direct('/opt/zapret2/ipset/zapret_hosts_auto_debug.log'), '') : Promise.resolve('')
		]);
	}).then(function(results) {
		var findResult = results[0] || {};
		var autohostlistLog = results[1] || '';

		var logdata = [];

		logdata.push({
			filename: '/opt/zapret2/ipset/zapret_hosts_auto_debug.log',
			cfgname: 'autohostlist',
			title: _('Autohostlist'),
			data: autohostlistLog,
			rows: getLineCount(autohostlistLog)
		});

		if (findResult.code === 0 && findResult.stdout) {
			var logFiles = findResult.stdout.trim().split('\n').filter(function(f) {
				return f && f.endsWith('.log');
			});

			logFiles.sort(function(a, b) {
				var aName = a.replace(/.*\//, '').replace(/\.log$/, '');
				var bName = b.replace(/.*\//, '').replace(/\.log$/, '');
				if (aName === 'main') return -1;
				if (bName === 'main') return 1;
				return aName.localeCompare(bName);
			});

			var readTasks = logFiles.map(function(path) {
				return L.resolveDefault(fs.read_direct(path), '');
			});

			return Promise.all(readTasks).then(function(contents) {
				for (var i = 0; i < logFiles.length; i++) {
					var path = logFiles[i];
					var content = contents[i] || '';
					var cfgname = path.replace(/.*\//, '').replace(/\.log$/, '');
					var title = formatTitle(cfgname);

					logdata.push({
						filename: path,
						cfgname: cfgname,
						title: title,
						data: content,
						rows: getLineCount(content)
					});
				}
				return logdata;
			});
		}

		return logdata;
	}).catch(function(e) {
		ui.addNotification(null, E('p', _('Unable to retrieve log files:') + ' ' + e.message));
		return [];
	});
}

function pollLog(path, textareaId) {
	var textarea = document.getElementById(textareaId);
	if (!textarea) return;

	L.resolveDefault(fs.stat(path), null).then(function(stat) {
		if (!stat) return;
		return fs.read_direct(path);
	}).then(function(content) {
		if (textarea && content) {
			textarea.value = content;
			textarea.scrollTop = textarea.scrollHeight;
		}
	}).catch(function() {});
}

function renderDebuglogUI() {
	var container = E('div', { 'class': 'cbi-section' }, [
		E('div', { 'class': 'cbi-value-description', 'style': 'margin-bottom: 1em;' },
			_('View debug logs from zapret2 service and custom scripts. Enable debug mode in Settings first.')),
		E('div', { 'id': 'debuglog-loading', 'style': 'padding: 20px; text-align: center;' }, _('Loading logs...'))
	]);

	retrieveLogs().then(function(logdata) {
		var loadingDiv = document.getElementById('debuglog-loading');
		if (loadingDiv) loadingDiv.remove();

		if (!logdata || logdata.length === 0) {
			container.appendChild(E('div', { 'class': 'cbi-value-description' },
				_('No log files found. Enable debug logging in Settings and restart the service.')));
			return;
		}

		var tabButtons = E('ul', { 'class': 'cbi-tabmenu' });
		var tabContents = E('div', { 'class': 'cbi-tabcontainer' });

		logdata.forEach(function(log, index) {
			var tabId = 'log-' + log.cfgname;
			var textareaId = 'textarea-' + log.cfgname;
			var isActive = index === 0;

			var li = E('li', {
				'class': isActive ? 'cbi-tab' : 'cbi-tab-disabled',
				'data-tab': tabId
			}, E('a', {
				'href': '#',
				'click': function(ev) {
					ev.preventDefault();

					tabButtons.querySelectorAll('li').forEach(function(el) {
						el.className = 'cbi-tab-disabled';
					});
					this.parentNode.className = 'cbi-tab';

					tabContents.querySelectorAll('.cbi-tabcontainer-content').forEach(function(el) {
						el.style.display = 'none';
					});
					var target = tabContents.querySelector('[data-tab-content="' + escapeCssSelector(tabId) + '"]');
					if (target) {
						target.style.display = 'block';
						var textarea = target.querySelector('textarea[data-path]');
						if (textarea) {
							pollLog(textarea.getAttribute('data-path'), textarea.id);
						}
					}
				}
			}, log.title));

			tabButtons.appendChild(li);

			var content = E('div', {
				'class': 'cbi-tabcontainer-content',
				'data-tab-content': tabId,
				'style': isActive ? 'display: block;' : 'display: none;'
			}, [
				E('div', { 'class': 'cbi-section' }, [
					E('p', { 'class': 'cbi-section-descr' }, _('Path:') + ' ' + log.filename),
					E('div', { 'style': 'margin-bottom: 10px;' }, [
						E('button', {
							'class': 'btn cbi-button cbi-button-neutral',
							'style': 'margin-right: 5px;',
							'data-textarea': textareaId,
							'click': function() {
								var ta = document.getElementById(this.getAttribute('data-textarea'));
								if (ta) ta.scrollTop = ta.scrollHeight;
							}
						}, _('Scroll to tail')),
						E('button', {
							'class': 'btn cbi-button cbi-button-neutral',
							'data-textarea': textareaId,
							'click': function() {
								var ta = document.getElementById(this.getAttribute('data-textarea'));
								if (ta) ta.scrollTop = 0;
							}
						}, _('Scroll to head'))
					]),
					E('textarea', {
						'id': textareaId,
						'class': 'cbi-input-textarea',
						'readonly': 'readonly',
						'wrap': 'off',
						'style': 'width: 100%; height: 400px; font-family: monospace; font-size: 12px;',
						'data-path': log.filename
					}, log.data || ''),
					E('div', { 'class': 'cbi-page-actions', 'style': 'margin-top: 10px;' }, [
						E('button', {
							'class': 'btn cbi-button cbi-button-action',
							'data-path': log.filename,
							'data-textarea': textareaId,
							'click': function() {
								pollLog(this.getAttribute('data-path'), this.getAttribute('data-textarea'));
							}
						}, _('Refresh')),
						' ',
						E('button', {
							'class': 'btn cbi-button cbi-button-reset',
							'data-path': log.filename,
							'data-textarea': textareaId,
							'click': function() {
								var path = this.getAttribute('data-path');
								var taId = this.getAttribute('data-textarea');
								if (!isAllowedLogPath(path)) {
									ui.addNotification(null, E('p', _('Invalid log path')), 'error');
									return;
								}
								L.resolveDefault(fs.remove(path), null).then(function() {
									var ta = document.getElementById(taId);
									if (ta) ta.value = '';
								});
							}
						}, _('Clear'))
					])
				])
			]);

			tabContents.appendChild(content);
		});

		var autoRefreshDiv = E('div', { 'style': 'margin-bottom: 1em;' }, [
			E('label', { 'class': 'cbi-checkbox' }, [
				E('input', {
					'type': 'checkbox',
					'id': 'auto-refresh',
					'change': function() {
						if (this.checked) {
							_pollFn = function() {
								var activeTab = tabContents.querySelector('.cbi-tabcontainer-content[style*="display: block"]');
								if (activeTab) {
									var textarea = activeTab.querySelector('textarea[data-path]');
									if (textarea) {
										pollLog(textarea.getAttribute('data-path'), textarea.id);
									}
								}
							};
							poll.add(_pollFn, 5);
						} else {
							if (_pollFn) {
								poll.remove(_pollFn);
								_pollFn = null;
							}
						}
					}
				}),
				' ',
				_('Auto-refresh every 5 seconds')
			])
		]);

		container.appendChild(autoRefreshDiv);
		container.appendChild(tabButtons);
		container.appendChild(tabContents);
	});

	return container;
}

return baseclass.extend({
	createDebuglogTab: function(map) {
		var s, o;

		// Use TypedSection with fake type for independent tab
		s = map.section(form.TypedSection, '_debuglog_tab', _('Debug Logs'));
		s.anonymous = true;
		s.addremove = false;
		s.cfgsections = function() { return ['_debuglog']; };

		o = s.option(form.DummyValue, '_debuglog_ui', ' ');
		o.rawhtml = true;
		o.cfgvalue = function() { return ''; };
		o.render = function() {
			return renderDebuglogUI();
		};
	}
});
