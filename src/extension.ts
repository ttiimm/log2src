/**
 * A "debugger" plug in for use with logdbg, a "logging debugger", that associates log files
 * with the source code that generated it. 
 * 
 * The extentions serves as a Debug Adapater and implements the applicable parts of the Debug Adapter
 * Protocol for logdbg. 
 * 
 * The extension is based off of vscode-mock-debug that Microsoft provides as an example debugger. This
 * module is a mash up of the extension.ts and activateMockDebug.ts.
 * 
 */

'use strict';

import * as vscode from 'vscode';
import { ProviderResult } from 'vscode';
import { DebugSession } from './debugAdapter';

const runMode: 'external' | 'server' | 'namedPipeServer' | 'inline' = 'inline';

export function activate(context: vscode.ExtensionContext) {
	vscode.window.showInformationMessage("TimTim here!");
	// The microsoft debug adapter extension had several ways of starting up, but the default inline method
	// seems easiest and so will focus on that initially. If there is need for other ways of starting, then
	// could look to the vscode-mock-debug for examples.
	switch (runMode) {

		case 'inline':
			// is there a way to do this in the package.json configuration instead?
			let factory = new InlineDebugAdapterFactory();
			context.subscriptions.push(vscode.debug.registerDebugAdapterDescriptorFactory('logdbg', factory));
			break;

		default:
			throw new Error('Unsupported runMode ' + runMode);
	}
}

export function deactivate() {
	// nothing to do
}


class InlineDebugAdapterFactory implements vscode.DebugAdapterDescriptorFactory {

	createDebugAdapterDescriptor(_session: vscode.DebugSession): ProviderResult<vscode.DebugAdapterDescriptor> {
		return new vscode.DebugAdapterInlineImplementation(new DebugSession());
	}

}