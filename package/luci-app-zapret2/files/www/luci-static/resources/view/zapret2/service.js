'use strict';
'require baseclass';
'require form';
'require uci';
'require rpc';
'require fs';
'require ui';
'require poll';
'require view.zapret2.main as main';
var BLOBS_DIR = '/opt/zapret2/files/fake';
var _statusPollFn = null;
var _statusUpdateInProgress = false;
function isValidHexMark(val) {
	if (!val) return false;
	return /^0x[0-9a-fA-F]{1,8}$/.test(val);
}
function parseQnumCounts(qnumInfo) {
	var mainCount = 0;
	var customCount = 0;
	if (qnumInfo && qnumInfo.trim()) {
		var lines = qnumInfo.trim().split('\n');
		lines.forEach(function(line) {
			var match = line.match(/qnum=(\d+)/);
			if (match) {
				var qnum = parseInt(match[1], 10);
				if (qnum < 1000) {
					mainCount++;
				} else {
					customCount++;
				}
			}
		});
	}
	return { main: mainCount, custom: customCount };
}
function updateServiceStatus() {
	if (_statusUpdateInProgress) return Promise.resolve();
	_statusUpdateInProgress = true;
	return Promise.all([
		L.resolveDefault(fs.exec('/usr/bin/pgrep', ['nfqws2']), { stdout: '' }),
		L.resolveDefault(fs.exec('/bin/sh', ['-c',
			'for pid in $(pgrep nfqws2 2>/dev/null); do ' +
			'cat /proc/$pid/cmdline 2>/dev/null | tr "\\0" " " | grep -o "qnum=[0-9]*" | head -1; ' +
			'done'
		]), { stdout: '' })
	]).then(function(results) {
		var pids = (results[0].stdout || '').trim();
		var processCount = pids ? pids.split('\n').length : 0;
		var qnumInfo = (results[1].stdout || '').trim();
		var running = processCount > 0;
		var counts = parseQnumCounts(qnumInfo);
		var mainCount = counts.main;
		var customCount = counts.custom;
		var statusText;
		if (running) {
			var parts = [];
			if (mainCount > 0) {
				parts.push('main');
			}
			if (customCount > 0) {
				parts.push(customCount + ' custom ' + (customCount === 1 ? 'script' : 'scripts'));
			}
			if (parts.length > 0) {
				statusText = _('Running') + ': ' + parts.join(' + ');
			} else {
				statusText = _('Running') + ': ' + processCount + ' ' + (processCount === 1 ? 'instance' : 'instances');
			}
		} else {
			statusText = _('Inactive');
		}
		var statusLabel = document.getElementById('zapret2-status-label');
		if (statusLabel) {
			statusLabel.className = running ? 'label label-success' : 'label label-danger';
			statusLabel.textContent = statusText;
		}
		var startBtn = document.getElementById('zapret2-start-btn');
		var stopBtn = document.getElementById('zapret2-stop-btn');
		if (startBtn) {
			startBtn.disabled = running;
			startBtn.classList.remove('spinning');
		}
		if (stopBtn) {
			stopBtn.disabled = !running;
			stopBtn.classList.remove('spinning');
		}
	}).finally(function() {
		_statusUpdateInProgress = false;
	});
}
var callInitAction = rpc.declare({
	object: 'luci',
	method: 'setInitAction',
	params: [ 'name', 'action' ],
	expect: { result: false }
});
function renderServiceStatus(initStatus, processCount, qnumInfo) {
	var enabled = initStatus.zapret2 && initStatus.zapret2.enabled;
	var running = false;
	var totalCount = 0;
	if (processCount > 0) {
		totalCount = processCount;
		running = true;
	}
	var counts = parseQnumCounts(qnumInfo);
	var mainCount = counts.main;
	var customCount = counts.custom;
	var restartBtn = E('button', {
		'class': 'btn cbi-button cbi-button-apply',
		'click': function() {
			this.classList.add('spinning');
			this.disabled = true;
			callInitAction('zapret2', 'restart').then(function() {
				window.location.reload();
			});
		}
	}, _('Restart'));
	var startBtn = E('button', {
		'id': 'zapret2-start-btn',
		'class': 'btn cbi-button cbi-button-positive',
		'click': function() {
			this.classList.add('spinning');
			this.disabled = true;
			callInitAction('zapret2', 'start').then(function() {
				updateServiceStatus();
			});
		}
	}, _('Start'));
	startBtn.disabled = running;
	var stopBtn = E('button', {
		'id': 'zapret2-stop-btn',
		'class': 'btn cbi-button cbi-button-reset',
		'click': function() {
			this.classList.add('spinning');
			this.disabled = true;
			callInitAction('zapret2', 'stop').then(function() {
				updateServiceStatus();
			});
		}
	}, _('Stop'));
	stopBtn.disabled = !running;
	var toggleBtn = E('button', {
		'class': enabled ? 'btn cbi-button cbi-button-reset' : 'btn cbi-button cbi-button-apply',
		'click': function() {
			this.classList.add('spinning');
			this.disabled = true;
			var action = enabled ? 'disable' : 'enable';
			callInitAction('zapret2', action).then(function() {
				window.location.reload();
			});
		}
	}, enabled ? _('Disable Autostart') : _('Enable Autostart'));
	var statusText;
	if (running) {
		var parts = [];
		if (mainCount > 0) {
			parts.push('main');
		}
		if (customCount > 0) {
			parts.push(customCount + ' custom ' + (customCount === 1 ? 'script' : 'scripts'));
		}
		if (parts.length > 0) {
			statusText = _('Running') + ': ' + parts.join(' + ');
		} else {
			statusText = _('Running') + ': ' + totalCount + ' ' + (totalCount === 1 ? 'instance' : 'instances');
		}
	} else {
		statusText = _('Inactive');
	}
	return E('div', { 'class': 'cbi-section', 'style': 'margin-bottom: 1em;' }, [
		E('table', { 'class': 'table' }, [
			E('tr', { 'class': 'tr' }, [
				E('td', { 'class': 'td left', 'width': '33%' }, _('Status')),
				E('td', { 'class': 'td right' },
					E('span', { 'id': 'zapret2-status-label', 'class': running ? 'label label-success' : 'label label-danger' },
						statusText
					)
				)
			]),
			E('tr', { 'class': 'tr' }, [
				E('td', { 'class': 'td left' }, _('Autostart')),
				E('td', { 'class': 'td right' },
					E('span', { 'class': enabled ? 'label label-success' : 'label label-warning' },
						enabled ? _('Enabled') : _('Disabled')
					)
				)
			])
		]),
		E('div', { 'style': 'display: flex; flex-wrap: wrap; justify-content: space-between; gap: 5px; margin-top: 1em;' }, [
			E('div', {}, [ toggleBtn ]),
			E('div', {}, [ restartBtn, ' ', startBtn, ' ', stopBtn ])
		])
	]);
}
function renderBlobsModal(map) {
	var blobs = uci.sections('zapret2', 'blob') || [];
	blobs.sort(function(a, b) {
		return a['.name'].localeCompare(b['.name']);
	});
	var checkboxes = [];
	var rows = blobs.map(function(blob) {
		var displayName = blob['.name'];
		var isEnabled = blob.enabled === '1';
		var sectionName = blob['.name'];
		var checkbox = E('input', {
			'type': 'checkbox',
			'checked': isEnabled ? 'checked' : null,
			'data-section': sectionName,
			'data-original': isEnabled ? '1' : '0'
		});
		checkboxes.push(checkbox);
		return E('tr', { 'class': 'tr' }, [
			E('td', { 'class': 'td center', 'style': 'width: 50px;' }, checkbox),
			E('td', { 'class': 'td left' }, displayName)
		]);
	});
	if (rows.length === 0) {
		rows.push(E('tr', { 'class': 'tr' }, [
			E('td', { 'class': 'td left', 'colspan': '2' }, _('No blob files found in ') + BLOBS_DIR)
		]));
	}
	var content = E('div', {}, [
		E('p', {}, _('Select blobs to load into nfqws2 at startup.')),
		E('p', {}, _('Use blob names in strategies with --lua-desync=fake:blob=<name>:... syntax')),
		E('p', { 'style': 'margin-top: 0.5em;' }, [
			_('Path') + ': ',
			E('code', {}, BLOBS_DIR)
		]),
		E('div', { 'style': 'max-height: 400px; overflow-y: auto; margin-top: 1em;' }, [
			E('table', { 'class': 'table', 'style': 'width: 100%;' }, [
				E('tr', { 'class': 'tr cbi-section-table-titles' }, [
					E('th', { 'class': 'th center', 'style': 'width: 50px;' }, _('Enable')),
					E('th', { 'class': 'th left' }, _('Name'))
				])
			].concat(rows))
		])
	]);
	ui.showModal(_('Blob Files'), [
		content,
		E('div', { 'style': 'display: flex; flex-wrap: wrap; justify-content: space-between; gap: 5px; margin-top: 1em;' }, [
			E('button', {
				'class': 'btn cbi-button',
				'click': ui.hideModal
			}, _('Cancel')),
			E('button', {
				'class': 'btn cbi-button-positive',
				'click': function() {
					checkboxes.forEach(function(cb) {
						var section = cb.getAttribute('data-section');
						var newValue = cb.checked ? '1' : '0';
						uci.set('zapret2', section, 'enabled', newValue);
					});
					main.reorderSections();
					uci.save().then(function() {
						ui.changes.init();
						ui.hideModal();
					});
				}
			}, _('Save'))
		])
	], 'cbi-modal');
}
function renderLuaModal(map) {
	var scripts = uci.sections('zapret2', 'luascript') || [];
	scripts.sort(function(a, b) {
		return a['.name'].localeCompare(b['.name']);
	});
	var checkboxes = [];
	var rows = scripts.map(function(script) {
		var displayName = script['.name'];
		var isEnabled = script.enabled === '1';
		var sectionName = script['.name'];
		var checkbox = E('input', {
			'type': 'checkbox',
			'checked': isEnabled ? 'checked' : null,
			'data-section': sectionName,
			'data-original': isEnabled ? '1' : '0'
		});
		checkboxes.push(checkbox);
		return E('tr', { 'class': 'tr' }, [
			E('td', { 'class': 'td center', 'style': 'width: 50px;' }, checkbox),
			E('td', { 'class': 'td left' }, displayName)
		]);
	});
	if (rows.length === 0) {
		rows.push(E('tr', { 'class': 'tr' }, [
			E('td', { 'class': 'td left', 'colspan': '2' }, _('No additional Lua scripts found in ') + main.LUA_DIR)
		]));
	}
	var content = E('div', {}, [
		E('p', {}, _('Select additional Lua scripts to load into nfqws2 at startup.')),
		E('p', {}, _('Base scripts (lib, antidpi, auto) are always loaded.')),
		E('p', { 'style': 'margin-top: 0.5em;' }, [
			_('Path') + ': ',
			E('code', {}, main.LUA_DIR)
		]),
		E('div', { 'style': 'max-height: 400px; overflow-y: auto; margin-top: 1em;' }, [
			E('table', { 'class': 'table', 'style': 'width: 100%;' }, [
				E('tr', { 'class': 'tr cbi-section-table-titles' }, [
					E('th', { 'class': 'th center', 'style': 'width: 50px;' }, _('Enable')),
					E('th', { 'class': 'th left' }, _('Name'))
				])
			].concat(rows))
		])
	]);
	ui.showModal(_('Lua Scripts'), [
		content,
		E('div', { 'style': 'display: flex; flex-wrap: wrap; justify-content: space-between; gap: 5px; margin-top: 1em;' }, [
			E('button', {
				'class': 'btn cbi-button',
				'click': ui.hideModal
			}, _('Cancel')),
			E('button', {
				'class': 'btn cbi-button-positive',
				'click': function() {
					checkboxes.forEach(function(cb) {
						var section = cb.getAttribute('data-section');
						var newValue = cb.checked ? '1' : '0';
						uci.set('zapret2', section, 'enabled', newValue);
					});
					main.reorderSections();
					uci.save().then(function() {
						ui.changes.init();
						ui.hideModal();
					});
				}
			}, _('Save'))
		])
	], 'cbi-modal');
}
function renderSettingsSection() {
	var G = 'display: grid; grid-template-columns: repeat(auto-fill, minmax(380px, 1fr)); gap: 6px 16px; width: 100%;';

	function cb(id, label, uciKey, checked) {
		var textSpan = E('span');
		textSpan.innerHTML = ' ' + label;
		return E('label', { 'class': 'bc2-checkbox' }, [
			E('input', {
				'type': 'checkbox',
				'id': id,
				'checked': checked ? 'checked' : null,
				'change': function() {
					uci.set('zapret2', 'main', uciKey, this.checked ? '1' : '0');
				}
			}),
			textSpan
		]);
	}

	function sel(id, uciKey, defaultVal, options) {
		var currentVal = uci.get('zapret2', 'main', uciKey) || defaultVal;
		var selectEl = E('select', {
			'class': 'cbi-input-select',
			'id': id,
			'change': function() {
				uci.set('zapret2', 'main', uciKey, this.value);
			}
		});
		options.forEach(function(opt) {
			selectEl.appendChild(E('option', {
				'value': opt[0],
				'selected': (opt[0] === currentVal) ? 'selected' : null
			}, opt[1]));
		});
		return selectEl;
	}

	function inp(id, uciKey, defaultVal, validateFn) {
		return E('input', {
			'type': 'text',
			'class': 'cbi-input-text',
			'id': id,
			'value': uci.get('zapret2', 'main', uciKey) || defaultVal,
			'placeholder': defaultVal,
			'change': function() {
				if (validateFn) {
					if (validateFn(this.value)) {
						this.classList.remove('cbi-input-invalid');
						uci.set('zapret2', 'main', uciKey, this.value);
					} else {
						this.classList.add('cbi-input-invalid');
					}
				} else {
					uci.set('zapret2', 'main', uciKey, this.value);
				}
			}
		});
	}

	function field(label, inputEl) {
		return E('div', { 'style': 'display: flex; align-items: center; gap: 8px;' }, [
			E('span', { 'style': 'width: 180px; min-width: 180px; flex-shrink: 0;' }, label),
			E('div', { 'style': 'flex: 1;' }, inputEl)
		]);
	}

	function header(text) {
		return E('div', { 'style': 'font-weight: bold; font-size: 90%; padding: 8px 0 2px; grid-column: 1 / -1;' }, text);
	}

	var grid = E('div', { 'style': G }, [

		// General
		header(_('General')),
		cb('setting_debug', _('Debug Mode'), 'debug',
			uci.get('zapret2', 'main', 'debug') === '1'),
		cb('setting_postnat', _('POSTNAT'), 'postnat',
			uci.get('zapret2', 'main', 'postnat') !== '0'),
		cb('setting_enable_ipv6', _('Enable IPv6'), 'enable_ipv6',
			uci.get('zapret2', 'main', 'enable_ipv6') === '1'),
		cb('setting_filter_ttl_expired_icmp', _('Filter TTL Expired ICMP'), 'filter_ttl_expired_icmp',
			uci.get('zapret2', 'main', 'filter_ttl_expired_icmp') !== '0'),
		field(_('Flow Offloading') + ': ', sel('setting_flowoffload', 'flowoffload', 'donttouch', [
			['donttouch', _('Don\'t touch')], ['none', _('Disabled')],
			['software', _('Software')], ['hardware', _('Hardware')]
		])),

		// Marks
		header(_('Marks')),
		field(_('Desync Mark') + ': ', inp('setting_desync_mark', 'desync_mark', '0x40000000', isValidHexMark)),
		field(_('Desync Mark Postnat') + ': ', inp('setting_desync_mark_postnat', 'desync_mark_postnat', '0x20000000', isValidHexMark)),
		field(_('Filter Mark') + ': ', inp('setting_filter_mark', 'filter_mark', '', function(v) { return !v || isValidHexMark(v); })),

		// Ports
		header(_('Ports')),
		field(_('TCP Ports') + ': ', inp('setting_nfqws_ports_tcp', 'nfqws_ports_tcp', '80,443')),
		field(_('UDP Ports') + ': ', inp('setting_nfqws_ports_udp', 'nfqws_ports_udp', '443')),

		// NFQWS
		header(_('NFQWS')),
		field(_('Outgoing Packet Limit') + ': ', inp('setting_nfqws_tcp_pkt_out', 'nfqws_tcp_pkt_out', '25')),
		field(_('Incoming Packet Limit') + ': ', inp('setting_nfqws_tcp_pkt_in', 'nfqws_tcp_pkt_in', '5')),
		field(_('Lua GC Interval') + ': ', inp('setting_lua_gc', 'lua_gc', '600')),
		field(_('Connection Tracking Timeouts') + ': ', inp('setting_ctrack_timeouts', 'ctrack_timeouts', ''))
	]);

	var container = E('div', { 'class': 'cbi-section', 'style': 'margin-top: 2em;' }, [
		E('h3', {}, _('Settings')),
		grid,
		E('div', { 'style': 'margin-top: 1em; margin-bottom: 1em;' }, [
			E('button', {
				'class': 'btn cbi-button cbi-button-reset',
				'click': function() {
					if (!confirm(_('Reset all settings to package defaults? Current configuration will be lost.'))) {
						return;
					}
					fs.exec('/opt/zapret2/reset-defaults.sh')
						.then(function() {
							window.location.reload();
						}).catch(function(err) {
							ui.addNotification(null, E('p', _('Failed to reset:') + ' ' + err.message), 'error');
						});
				}
			}, _('Reset to Defaults'))
		])
	]);

	return container;
}
return baseclass.extend({
	renderServiceControls: function(initStatus) {
		var processCount = main.getProcessCount();
		var qnumInfo = main.getQnumInfo();
		var panel = renderServiceStatus(initStatus, processCount, qnumInfo);
		if (!_statusPollFn) {
			_statusPollFn = updateServiceStatus;
			poll.add(_statusPollFn, 5);
		}
		return panel;
	},
	createServiceTab: function(map, initStatus) {
		main.injectStyles();
		var s, o;
		var processCount = main.getProcessCount();
		var qnumInfo = main.getQnumInfo();
		s = map.section(form.GridSection, 'strategy', _('Settings'));
		s.tabName = _('Zapret2');
		s.addremove = true;
		s.anonymous = false;
		s.sortable = true;
		s.nodescriptions = true;
		var originalRender = s.render;
		s.render = function() {
			var self = this;
			return originalRender.apply(this, arguments).then(function(el) {
				var strategiesHeader = E('h3', { 'style': 'margin-top: 0.5em; margin-bottom: 0.5em;' }, _('Strategies'));
				el.insertBefore(strategiesHeader, el.firstChild);
				var settingsEl = renderSettingsSection();
				el.appendChild(settingsEl);
				el.querySelectorAll('a[href="#"]').forEach(function(link) {
					if (link.textContent === 'strategy') {
						link.textContent = _('Settings');
					}
				});
				// Sync data-name from td cells to th headers for CSS responsive hiding
				var firstRow = el.querySelector('tr.cbi-section-table-row');
				var headerRow = el.querySelector('tr.cbi-section-table-titles');
				if (firstRow && headerRow) {
					var tds = firstRow.querySelectorAll('td[data-name]');
					var ths = headerRow.querySelectorAll('th.cbi-section-table-cell');
					tds.forEach(function(td) {
						var name = td.getAttribute('data-name');
						var idx = Array.prototype.indexOf.call(td.parentNode.children, td);
						if (ths[idx])
							ths[idx].setAttribute('data-name', name);
					});
				}
				return el;
			});
		};
		s.renderSectionAdd = function(extra_class) {
			var el = form.GridSection.prototype.renderSectionAdd.apply(this, arguments);
			var sectionMap = this.map;
			var blobsBtn = E('button', {
				'class': 'btn cbi-button',
				'style': 'margin-left: 10px;',
				'click': function(ev) {
					ev.preventDefault();
					renderBlobsModal(sectionMap);
				}
			}, _('Blobs'));
			el.appendChild(blobsBtn);
			var luaBtn = E('button', {
				'class': 'btn cbi-button',
				'style': 'margin-left: 10px;',
				'click': function(ev) {
					ev.preventDefault();
					renderLuaModal(sectionMap);
				}
			}, _('Lua'));
			el.appendChild(luaBtn);
			return el;
		};
		s.handleAdd = function(ev, name) {
			var config = this.uciconfig || this.map.config;
			var result = main.validateStrategyName(name, config, null);
			if (result !== true) {
				ui.addNotification(null, E('p', result), 'error');
				return Promise.resolve();
			}
			return form.GridSection.prototype.handleAdd.apply(this, [ev, name.trim()]);
		};
		s.handleRemove = function(section_id, ev) {
			uci.remove('zapret2', section_id);
			return this.map.save();
		};
		s.renderRowActions = function(section_id) {
			var tdEl = form.GridSection.prototype.renderRowActions.apply(this, [section_id]);
			var gridSection = this;
			var copyBtn = E('button', {
				'class': 'btn cbi-button cbi-button-action',
				'title': _('Copy strategy')
			}, _('Copy'));
			copyBtn.addEventListener('click', function(ev) {
				ev.preventDefault();
				ev.stopPropagation();
				var config = gridSection.uciconfig || gridSection.map.config;
				var newName = section_id + '_copy';
				var counter = 1;
				while (uci.get(config, newName)) {
					newName = section_id + '_copy' + counter;
					counter++;
				}
				var sections = uci.sections(config, 'strategy');
				var sourceSection = null;
				for (var i = 0; i < sections.length; i++) {
					if (sections[i]['.name'] === section_id) {
						sourceSection = sections[i];
						break;
					}
				}
				uci.add(config, 'strategy', newName);
				if (sourceSection) {
					for (var key in sourceSection) {
						if (key.charAt(0) !== '.') {
							uci.set(config, newName, key, sourceSection[key]);
						}
					}
				}
				return gridSection.map.save().then(function() {
					return gridSection.map.load();
				}).then(function() {
					return gridSection.map.reset();
				});
			});
			var btnContainer = tdEl.lastElementChild || tdEl;
			if (btnContainer.querySelector) {
				var existingBtns = btnContainer.querySelectorAll('button');
				if (existingBtns.length > 0) {
					var lastBtn = existingBtns[existingBtns.length - 1];
					lastBtn.parentNode.insertBefore(copyBtn, lastBtn);
					lastBtn.parentNode.insertBefore(document.createTextNode(' '), lastBtn);
				} else {
					btnContainer.appendChild(copyBtn);
				}
			} else {
				tdEl.appendChild(copyBtn);
			}
			return tdEl;
		};
		o = s.option(form.Value, '_name', _('Name'));
		o.modalonly = true;
		o.rmempty = false;
		o.validate = function(section_id, value) {
			var config = this.section.uciconfig || this.map.config;
			return main.validateStrategyName(value, config, section_id);
		};
		o.cfgvalue = function(section_id) {
			return section_id;
		};
		o.write = function(section_id, value) {
			if (value && value !== section_id) {
				var config = this.section.uciconfig || this.map.config;
				var sections = uci.sections(config, 'strategy');
				var sourceSection = null;
				for (var i = 0; i < sections.length; i++) {
					if (sections[i]['.name'] === section_id) {
						sourceSection = sections[i];
						break;
					}
				}
				if (!sourceSection) {
					ui.addNotification(null, E('p', _('Error: Source strategy not found')), 'error');
					return Promise.reject();
				}
				uci.add(config, 'strategy', value);
				for (var key in sourceSection) {
					if (key.charAt(0) !== '.' && key !== '_name') {
						uci.set(config, value, key, sourceSection[key]);
					}
				}
				uci.remove(config, section_id);
			}
		};
		o = s.option(form.Flag, 'enabled', _('Enabled'));
		o.editable = true;
		o.default = '1';
		o.rmempty = false;
		o = s.option(form.Value, 'port', _('Port'));
		o.placeholder = '443 or 80,443 or ~443 or 1000-2000';
		o.default = '443';
		o.validate = function(section_id, value) {
			if (!value || value.length === 0)
				return _('Port is required');
			if (!/^~?[0-9,\-]+$/.test(value))
				return _('Only digits, comma, hyphen and ~ allowed');
			var clean = value.replace(/^~/, '');
			var parts = clean.split(',');
			for (var i = 0; i < parts.length; i++) {
				var part = parts[i].trim();
				if (part.indexOf('-') !== -1) {
					var range = part.split('-');
					if (range.length !== 2)
						return _('Invalid port range format');
					var start = parseInt(range[0], 10);
					var end = parseInt(range[1], 10);
					if (isNaN(start) || isNaN(end) || start < 1 || start > 65535 || end < 1 || end > 65535 || start > end)
						return _('Port must be between 1 and 65535');
				} else {
					var port = parseInt(part, 10);
					if (isNaN(port) || port < 1 || port > 65535)
						return _('Port must be between 1 and 65535');
				}
			}
			return true;
		};
		o = s.option(form.MultiValue, 'filter_l3', _('Filter L3'),
			_('IP version filter'));
		o.value('ipv4', 'IPv4');
		o.value('ipv6', 'IPv6');
		o.default = 'ipv4';
		o.rmempty = false;
		o.validate = function(section_id, value) {
			if (!value || value.length === 0)
				return _('At least one IP version must be selected');
			return true;
		};
		o = s.option(form.MultiValue, 'protocol', _('Protocol'),
			_('Select one or both protocols for this strategy'));
		o.value('tcp', 'TCP');
		o.value('udp', 'UDP');
		o.default = 'tcp';
		o.rmempty = false;
		o.validate = function(section_id, value) {
			if (!value || value.length === 0)
				return _('At least one protocol must be selected');
			return true;
		};
		o = s.option(form.MultiValue, 'filter_l7', _('Filter L7'),
			_('Layer 7 protocol detection'));
		o.value('all', _('All'));
		o.value('unknown', _('Unknown'));
		o.value('known', _('Known'));
		o.value('tls', 'TLS');
		o.value('http', 'HTTP');
		o.value('quic', 'QUIC');
		o.value('dns', 'DNS');
		o.value('stun', 'STUN');
		o.value('wireguard', 'WireGuard');
		o.value('dht', 'DHT');
		o.value('discord', 'Discord');
		o.value('xmpp', 'XMPP');
		o.value('mtproto', 'MTProto');
		o.rmempty = true;
		o = s.option(form.DummyValue, '_lists', _('Lists'));
		o.modalonly = false;
		o.textvalue = function(section_id) {
			var hosts = L.toArray(uci.get('zapret2', section_id, 'hostlist'));
			var ipsets = L.toArray(uci.get('zapret2', section_id, 'ipset'));
			var parts = [];
			if (hosts.length > 0) {
				var names = hosts.map(function(h) {
					return h.replace(/^list_/, '').replace(/^hosts_/, '');
				});
				parts.push('host: ' + names.join(', '));
			}
			if (ipsets.length > 0) {
				var names = ipsets.map(function(h) {
					return h.replace(/^list_/, '').replace(/^ipset_/, '');
				});
				parts.push('ipset: ' + names.join(', '));
			}
			return parts.length > 0 ? parts.join('\n') : '-';
		};

		o = s.option(form.MultiValue, 'hostlist', _('Host lists'),
			_('Select domain lists (zapret_hosts_*.txt) to apply this strategy to'));
		o.rmempty = true;
		o.modalonly = true;
		o.optional = true;
		o.load = function(section_id) {
			this.keylist = [];
			this.vallist = [];
			main.populateListOptions(this, 'zapret_hosts_', 'hosts_', _('-- No host lists available --'));
			return form.MultiValue.prototype.load.apply(this, [section_id]);
		};
		o.validate = function(section_id, value) {
			var exclude = L.toArray(uci.get('zapret2', section_id, 'hostlist_exclude'));
			var include = L.toArray(value);
			for (var i = 0; i < include.length; i++) {
				if (exclude.indexOf(include[i]) !== -1)
					return _('List "%s" is in both include and exclude').replace('%s', include[i]);
			}
			return true;
		};
		o = s.option(form.MultiValue, 'ipset', _('IP sets'),
			_('Select IP sets (zapret_ipset_*.txt) to apply this strategy to'));
		o.rmempty = true;
		o.modalonly = true;
		o.optional = true;
		o.load = function(section_id) {
			this.keylist = [];
			this.vallist = [];
			main.populateListOptions(this, 'zapret_ipset_', 'ipset_', _('-- No IP sets available --'));
			return form.MultiValue.prototype.load.apply(this, [section_id]);
		};
		o.validate = function(section_id, value) {
			var exclude = L.toArray(uci.get('zapret2', section_id, 'ipset_exclude'));
			var include = L.toArray(value);
			for (var i = 0; i < include.length; i++) {
				if (exclude.indexOf(include[i]) !== -1)
					return _('List "%s" is in both include and exclude').replace('%s', include[i]);
			}
			return true;
		};
		o = s.option(form.MultiValue, 'hostlist_exclude', _('Exclude lists'),
			_('Select domain lists (zapret_hosts_*.txt) to EXCLUDE from this strategy'));
		o.rmempty = true;
		o.modalonly = true;
		o.optional = true;
		o.load = function(section_id) {
			this.keylist = [];
			this.vallist = [];
			main.populateListOptions(this, 'zapret_hosts_', 'hosts_', _('-- No host lists available --'));
			return form.MultiValue.prototype.load.apply(this, [section_id]);
		};
		o.validate = function(section_id, value) {
			var include = L.toArray(uci.get('zapret2', section_id, 'hostlist'));
			var exclude = L.toArray(value);
			for (var i = 0; i < exclude.length; i++) {
				if (include.indexOf(exclude[i]) !== -1)
					return _('List "%s" is in both include and exclude').replace('%s', exclude[i]);
			}
			return true;
		};
		o = s.option(form.MultiValue, 'ipset_exclude', _('Exclude IP sets'),
			_('Select IP sets (zapret_ipset_*.txt) to EXCLUDE from this strategy'));
		o.rmempty = true;
		o.modalonly = true;
		o.optional = true;
		o.load = function(section_id) {
			this.keylist = [];
			this.vallist = [];
			main.populateListOptions(this, 'zapret_ipset_', 'ipset_', _('-- No IP sets available --'));
			return form.MultiValue.prototype.load.apply(this, [section_id]);
		};
		o.validate = function(section_id, value) {
			var include = L.toArray(uci.get('zapret2', section_id, 'ipset'));
			var exclude = L.toArray(value);
			for (var i = 0; i < exclude.length; i++) {
				if (include.indexOf(exclude[i]) !== -1)
					return _('List "%s" is in both include and exclude').replace('%s', exclude[i]);
			}
			return true;
		};
		o = s.option(form.Flag, 'autohostlist', _('Autohostlist'),
			_('Enable automatic host detection for this strategy'));
		o.rmempty = true;
		o.modalonly = true;
		o.default = '0';
		o.enabled = '1';
		o.disabled = '0';
		o = s.option(form.Flag, 'autohostlist_debuglog', _('Autohostlist Debug Log'),
			_('Enable debug logging for autohostlist to /opt/zapret2/ipset/zapret_hosts_auto_debug.log'));
		o.rmempty = true;
		o.modalonly = true;
		o.default = '0';
		o.enabled = '1';
		o.disabled = '0';
		o.depends('autohostlist', '1');
		o = s.option(form.TextValue, 'script', _('NFQWS Options'),
			_('Additional nfqws command line options for this strategy'));
		o.rows = 20;
		o.monospace = true;
		o.rmempty = true;
		o.modalonly = true;
	}
});
