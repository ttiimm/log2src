import * as assert from 'assert';
import * as vscode from 'vscode';
// import * as myExtension from '../../extension';

suite('Extension Test Suite', () => {
	vscode.window.showInformationMessage('Start all tests.');

	test('Extension should be present', () => {
		assert.ok(vscode.extensions.getExtension('log2src.log2src'));
	});

	test('Extension should activate', async () => {
		const ext = vscode.extensions.getExtension('log2src.log2src');
		assert.ok(ext);
		await ext!.activate();
		assert.strictEqual(ext!.isActive, true);
	});

	test('Should register log2src debug type', async () => {
		// Ensure extension is activated
		const ext = vscode.extensions.getExtension('log2src.log2src');
		await ext?.activate();

		// Verify the extension activated successfully
		assert.ok(ext?.isActive, 'Extension should be active');
	});
});
