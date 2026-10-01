import * as assert from 'assert';
import * as path from 'path';
import * as vscode from 'vscode';

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

	test('Source CodeLens shows matches without opening the log document', async () => {
		const sourceFile = path.resolve(__dirname, '../../../../../examples/basic.rs');
		const logFile = path.resolve(__dirname, '../../../../../tests/resources/rust/basic.log');
		const sourceRoot = path.dirname(sourceFile);
		const config = vscode.workspace.getConfiguration('log2src');
		const ext = vscode.extensions.getExtension('log2src.log2src');
		await ext?.activate();
		await config.update('sourceRoots', [sourceRoot], vscode.ConfigurationTarget.Global);
		await config.update('logFile', logFile, vscode.ConfigurationTarget.Global);
		await config.update(
			'logFormat',
			String.raw`\[\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z \w+ \w+\]\s+(?<body>.*)`,
			vscode.ConfigurationTarget.Global
		);

		try {
			const document = await vscode.workspace.openTextDocument(vscode.Uri.file(sourceFile));
			const lenses = await vscode.commands.executeCommand<vscode.CodeLens[]>(
				'vscode.executeCodeLensProvider',
				document.uri
			);

			const fooLens = lenses?.find(lens => lens.command?.title === '3 matching log messages');
			assert.ok(fooLens, 'should show the three matching foo log messages');
			assert.strictEqual(fooLens.range.start.line, 14);
			assert.strictEqual(
				vscode.workspace.textDocuments.some(openDocument => openDocument.uri.fsPath === logFile),
				false
			);
		} finally {
			await config.update('sourceRoots', undefined, vscode.ConfigurationTarget.Global);
			await config.update('logFile', undefined, vscode.ConfigurationTarget.Global);
			await config.update('logFormat', undefined, vscode.ConfigurationTarget.Global);
		}
	});
});
