/**
 * debugAdapter.ts implements the Debug Adapter protocol and integrates it with the logdbg
 * "debugger".
 * 
 * Care should be given to make sure that this module is independent from VS Code so that it
 * could potentially be used in other IDE.
 */

import { LoggingDebugSession } from '@vscode/debugadapter';
import { DebugProtocol } from '@vscode/debugprotocol';


export class DebugSession extends LoggingDebugSession {

    /**
     * Create a new debug adapter to use with a debug session.
     */
    public constructor() {
        super("logdbg-dap.txt");

        this.setDebuggerLinesStartAt1(true);
        this.setDebuggerColumnsStartAt1(true);
    }

    /**
	 * The 'initialize' request is the first request called by the frontend
	 * to interrogate the features the debug adapter provides.
	 */
	protected initializeRequest(response: DebugProtocol.InitializeResponse, args: DebugProtocol.InitializeRequestArguments): void {
        response.body = response.body || {};
        response.body.supportsStepBack = true;
        
        this.sendResponse(response);
    }
}
