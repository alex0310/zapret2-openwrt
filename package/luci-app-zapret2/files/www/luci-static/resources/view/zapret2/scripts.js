'use strict';
'require baseclass';
'require form';
'require uci';
'require fs';
'require ui';
var SCRIPTS_DIR = '/opt/zapret2/init.d/openwrt/custom.d';
var _scriptsData = [];
var _scriptsContainer = null;
function isValidScriptName(name) {
	if (!name || name.length === 0 || name.length > 50) return false;
	if (!/^[a-zA-Z0-9_]+$/.test(name)) return false;
	return true;
}
function isValidFilename(name) {
	if (!name || name.indexOf('..') !== -1 || name.indexOf('/') !== -1) return false;
	if (name.startsWith('.') || name.startsWith('-')) return false;
	if (!/^[a-zA-Z0-9][a-zA-Z0-9_\-\.]*\.sh$/.test(name)) return false;
	if (name.length > 64) return false;
	return true;
}

function isPathInDir(path, dir) {
	if (!path || !dir) return false;
	if (path.indexOf('\0') !== -1) return false;
	var normalizedPath = path.replace(/\/+/g, '/');
	var normalizedDir = dir.replace(/\/+$/, '') + '/';
	if (!normalizedPath.startsWith(normalizedDir)) return false;
	if (normalizedPath.indexOf('..') !== -1) return false;
	var filename = normalizedPath.substring(normalizedDir.length);
	if (filename.indexOf('/') !== -1) return false;
	return true;
}

function isValidOldPath(path) {
	if (!isPathInDir(path, SCRIPTS_DIR)) return false;
	var filename = path.replace(/.*\//, '');
	return isValidFilename(filename);
}
function parseFilename(filename) {
	var match = filename.match(/^(\d+)-(.+)\.sh$/);
	if (match) return { order: match[1], name: match[2] };
	match = filename.match(/^(.+)\.sh$/);
	if (match) return { order: '', name: match[1] };
	return { order: '', name: filename };
}
function buildFilename(order, name) {
	if (order && order.trim()) return order.trim() + '-' + name + '.sh';
	return name + '.sh';
}
function loadScripts() {
	return L.resolveDefault(fs.list(SCRIPTS_DIR), []).then(function(files) {
		_scriptsData = [];
		(files || []).forEach(function(file) {
			if (file.name && file.name.endsWith('.sh') && file.name !== '.keep') {
				var parsed = parseFilename(file.name);
				_scriptsData.push({
					filename: file.name,
					path: SCRIPTS_DIR + '/' + file.name,
					order: parsed.order,
					name: parsed.name
				});
			}
		});
		_scriptsData.sort(function(a, b) {
			return a.filename.localeCompare(b.filename);
		});
		return _scriptsData;
	});
}
function clearElement(el) {
	while (el.firstChild) {
		el.removeChild(el.firstChild);
	}
}
function safeParseScriptData(jsonStr) {
	try {
		var data = JSON.parse(jsonStr);
		if (data && typeof data.filename === 'string' && typeof data.path === 'string') {
			return data;
		}
	} catch (e) {}
	return null;
}
function renderScriptsTable() {
	if (!_scriptsContainer) return;
	clearElement(_scriptsContainer);
	if (_scriptsData.length > 0) {
		var table = E('table', { 'class': 'table' }, [
			E('tr', { 'class': 'tr cbi-section-table-titles' }, [
				E('th', { 'class': 'th' }, _('Order')),
				E('th', { 'class': 'th' }, _('Name')),
				E('th', { 'class': 'th' }, _('Filename')),
				E('th', { 'class': 'th cbi-section-actions' }, '')
			])
		]);
		_scriptsData.forEach(function(script) {
			var row = E('tr', { 'class': 'tr' }, [
				E('td', { 'class': 'td left' }, script.order || '-'),
				E('td', { 'class': 'td left' }, script.name),
				E('td', { 'class': 'td left' }, script.filename),
				E('td', { 'class': 'td right cbi-section-actions' }, [
					E('div', {}, [
						E('button', {
							'class': 'cbi-button cbi-button-edit',
							'data-script': JSON.stringify(script),
							'click': function() {
								var data = safeParseScriptData(this.getAttribute('data-script'));
								if (data) handleEdit(data);
							}
						}, _('Edit')),
						' ',
						E('button', {
							'class': 'cbi-button cbi-button-remove',
							'data-path': script.path,
							'click': function() {
								handleRemove(this.getAttribute('data-path'));
							}
						}, _('Delete'))
					])
				])
			]);
			table.appendChild(row);
		});
		_scriptsContainer.appendChild(table);
	} else {
		_scriptsContainer.appendChild(E('div', { 'class': 'cbi-value-description' },
			_('No custom scripts found. Click Add to create one.')));
	}
	var addRow = E('div', { 'class': 'cbi-section-create', 'style': 'margin-top: 1em;' }, [
		E('input', {
			'type': 'text',
			'class': 'cbi-section-create-name',
			'id': 'new_script_name',
			'placeholder': _('Script name (letters, numbers, underscores)')
		}),
		E('button', {
			'class': 'cbi-button cbi-button-add',
			'click': handleAdd
		}, _('Add'))
	]);
	_scriptsContainer.appendChild(addRow);
}
function handleAdd() {
	var input = document.getElementById('new_script_name');
	var name = input ? input.value.trim() : '';
	if (!isValidScriptName(name)) {
		ui.addNotification(null, E('p', _('Invalid name. Use only ASCII letters, numbers and underscores (max 50 chars).')), 'error');
		return;
	}
	var filename = '50-' + name + '.sh';
	var path = SCRIPTS_DIR + '/' + filename;
	var exists = _scriptsData.some(function(s) {
		return s.name === name || s.filename === filename;
	});
	if (exists) {
		ui.addNotification(null, E('p', _('Script with this name already exists.')), 'error');
		return;
	}
	var defaultTemplate = '#!/bin/sh\n# Custom nfqws2 script\n# Variables available: $NFQWS2, $DESYNC_MARK, etc.\n\n';
	fs.write(path, defaultTemplate).then(function() {
		return fs.exec('/bin/chmod', ['755', path]);
	}).then(function() {
		input.value = '';
		var newScript = { filename: filename, path: path, order: '50', name: name };
		handleEdit(newScript);
	}).catch(function(err) {
		ui.addNotification(null, E('p', _('Failed to create script:') + ' ' + err.message), 'error');
	});
}
function handleEdit(script) {
	L.resolveDefault(fs.read(script.path), '').then(function(content) {
		var parsed = parseFilename(script.filename);
		var body = E('div', {}, [
			E('div', { 'class': 'cbi-value' }, [
				E('label', { 'class': 'cbi-value-title' }, _('Order')),
				E('div', { 'class': 'cbi-value-field' }, [
					E('input', { 'type': 'text', 'id': 'edit_order', 'class': 'cbi-input-text', 'value': parsed.order, 'placeholder': '50' }),
					E('div', { 'class': 'cbi-value-description' }, _('Scripts execute in alphabetical order. Leave empty for no prefix.'))
				])
			]),
			E('div', { 'class': 'cbi-value' }, [
				E('label', { 'class': 'cbi-value-title' }, _('Name')),
				E('div', { 'class': 'cbi-value-field' }, [
					E('input', { 'type': 'text', 'id': 'edit_name', 'class': 'cbi-input-text', 'value': parsed.name }),
					E('div', { 'class': 'cbi-value-description' }, _('Script name (without order prefix and .sh extension)'))
				])
			]),
			E('div', { 'class': 'cbi-value' }, [
				E('label', { 'class': 'cbi-value-title' }, _('Content')),
				E('div', { 'class': 'cbi-value-field' }, [
					E('textarea', { 'id': 'edit_content', 'class': 'cbi-input-textarea', 'rows': '20', 'style': 'width: 100%; font-family: monospace;' }, content || '#!/bin/sh\n')
				])
			])
		]);
		ui.showModal(_('Edit Script'), [
			body,
			E('div', { 'class': 'right', 'style': 'margin-top: 1em;' }, [
				E('button', { 'class': 'cbi-button', 'click': ui.hideModal }, _('Cancel')),
				' ',
				E('button', {
					'class': 'cbi-button cbi-button-positive',
					'data-script': JSON.stringify(script),
					'click': function() {
						var data = safeParseScriptData(this.getAttribute('data-script'));
						if (data) handleSaveEdit(data);
					}
				}, _('Save'))
			])
		], 'cbi-modal');
	});
}
function handleSaveEdit(script) {
	var newOrder = document.getElementById('edit_order').value.trim();
	var newName = document.getElementById('edit_name').value.trim();
	var content = document.getElementById('edit_content').value;
	if (!isValidScriptName(newName)) {
		ui.addNotification(null, E('p', _('Invalid name. Use only ASCII letters, numbers and underscores.')), 'error');
		return;
	}
	if (newOrder && !/^\d+$/.test(newOrder)) {
		ui.addNotification(null, E('p', _('Order must be a number.')), 'error');
		return;
	}
	var newFilename = buildFilename(newOrder, newName);
	if (!isValidFilename(newFilename)) {
		ui.addNotification(null, E('p', _('Invalid filename.')), 'error');
		return;
	}
	var newPath = SCRIPTS_DIR + '/' + newFilename;
	var oldPath = script.path;
	if (!isPathInDir(newPath, SCRIPTS_DIR) || !isValidOldPath(oldPath)) {
		ui.addNotification(null, E('p', _('Invalid path.')), 'error');
		return;
	}
	loadScripts().then(function() {
		if (newFilename !== script.filename) {
			var conflict = _scriptsData.some(function(s) {
				return s.filename === newFilename;
			});
			if (conflict) {
				ui.addNotification(null, E('p', _('A script with this filename already exists.')), 'error');
				return Promise.reject(new Error('conflict'));
			}
		}
		return fs.write(newPath, content || '');
	}).then(function() {
		return fs.exec('/bin/chmod', ['755', newPath]);
	}).then(function() {
		if (oldPath !== newPath) {
			return L.resolveDefault(fs.remove(oldPath), null);
		}
	}).then(function() {
		ui.hideModal();
		ui.addNotification(null, E('p', _('Script saved successfully.')), 'info');
		return loadScripts();
	}).then(function() {
		renderScriptsTable();
	}).catch(function(err) {
		if (err && err.message !== 'conflict') {
			ui.addNotification(null, E('p', _('Failed to save script:') + ' ' + err.message), 'error');
		}
	});
}
function handleRemove(path) {
	if (!confirm(_('Are you sure you want to delete this script?'))) return;
	if (!isPathInDir(path, SCRIPTS_DIR)) {
		ui.addNotification(null, E('p', _('Invalid path.')), 'error');
		return;
	}
	fs.remove(path).then(function() {
		ui.addNotification(null, E('p', _('Script deleted.')), 'info');
		return loadScripts();
	}).then(function() {
		renderScriptsTable();
	}).catch(function(err) {
		ui.addNotification(null, E('p', _('Failed to delete script:') + ' ' + err.message), 'error');
	});
}
return baseclass.extend({
	createScriptsTab: function(map) {
		var s, o;
		s = map.section(form.TypedSection, '_scripts_tab', _('Scripts'));
		s.anonymous = true;
		s.addremove = false;
		s.cfgsections = function() { return ['_scripts']; };
		o = s.option(form.DummyValue, '_enable', _('Enable Custom Scripts'));
		o.rawhtml = true;
		o.cfgvalue = function() { return ''; };
		o.render = function() {
			var enabled = uci.get('zapret2', 'main', 'custom_scripts') !== '0';
			var checkbox = E('input', {
				'type': 'checkbox',
				'id': 'custom_scripts_enable',
				'checked': enabled ? 'checked' : null,
				'change': function() {
					uci.set('zapret2', 'main', 'custom_scripts', this.checked ? '1' : '0');
				}
			});
			return E('div', { 'class': 'cbi-value' }, [
				E('label', { 'class': 'cbi-value-title' }, _('Enable Custom Scripts')),
				E('div', { 'class': 'cbi-value-field' }, [
					checkbox
				])
			]);
		};
		o = s.option(form.DummyValue, '_scripts_list', _('Scripts List'));
		o.rawhtml = true;
		o.cfgvalue = function() { return ''; };
		o.render = function() {
			_scriptsContainer = E('div', { 'id': 'scripts-table-container' });
			loadScripts().then(function() {
				renderScriptsTable();
			});
			return _scriptsContainer;
		};
	}
});
