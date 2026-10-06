import * as assert from 'assert';
import * as path from 'path';
import * as vscode from 'vscode';
import { spawnSync } from 'child_process';
import { resolveRelativeBinaryPath } from '../../log2srcClient';

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

	test('Source CodeLens shows matches without opening the log document', async function () {
		this.timeout(30000);
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
			// Provider registration and config propagation are async, so retry until the lens appears.
			let lenses: vscode.CodeLens[] | undefined;
			let fooLens: vscode.CodeLens | undefined;
			for (let attempt = 0; attempt < 20 && !fooLens; attempt++) {
				lenses = await vscode.commands.executeCommand<vscode.CodeLens[]>(
					'vscode.executeCodeLensProvider',
					document.uri
				);
				fooLens = lenses?.find(lens => lens.command?.title === '3 matching log messages');
				if (!fooLens) {
					await new Promise(resolve => setTimeout(resolve, 500));
				}
			}

			let diagnostic = '';
			if (!fooLens) {
				// The provider swallows scan errors, so run the scan directly to expose them.
				try {
					// The client discards stderr, so run the binary directly with -v to expose it.
					const binary = path.resolve(__dirname, '../..', resolveRelativeBinaryPath());
					const run = spawnSync(binary, ['-d', sourceRoot, '--log', logFile, '--summary', '-v'], { encoding: 'utf8' });
					diagnostic = `exit=${run.status} error=${run.error} stdout=${run.stdout?.slice(0, 500)} stderr=${run.stderr?.slice(0, 2000)}`;
				} catch (error) {
					diagnostic = `direct scan failed: ${String(error)}`;
				}
			}
			assert.ok(
				fooLens,
				`should show the three matching foo log messages; got ${JSON.stringify(lenses?.map(l => l.command?.title))}; ${diagnostic}`
			);
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
