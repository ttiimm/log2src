/**
 * logNavigation.ts implements a "click" a log line to navigate to its source.
 */

import * as vscode from 'vscode';
import * as path from 'path';

import { LogMatchIndex } from './logMatchIndex';
import { Log2srcClient, SourceRef } from './log2srcClient';
import { OutputSink } from './debugAdapter';

export const LOG_LANGUAGE_ID = 'log2src';
const CONFIGURE_EXTENSION = 'log2src.configure';
const OPEN_SOURCE_LOCATION_COMMAND = 'log2src.openSourceLocation';
const GO_TO_SOURCE_AT_CURSOR_COMMAND = 'log2src.goToSourceAtCursor';

interface NavigationConfig {
    sourceDirs: string[];
    logFormat?: string;
}

function getNavigationConfig(): NavigationConfig {
    const config = vscode.workspace.getConfiguration('log2src');
    return {
        sourceDirs: config.get<string[]>('sourceRoots', []),
        logFormat: config.get<string>('logFormat') || undefined,
    };
}

function sourceRefTitle(srcRef: SourceRef): string {
    return `Go to source: ${path.basename(srcRef.sourcePath)}:${srcRef.lineNumber}`;
}

async function openSourceLocation(srcRef: SourceRef): Promise<void> {
    const doc = await vscode.workspace.openTextDocument(srcRef.sourcePath);
    const editor = await vscode.window.showTextDocument(doc, { viewColumn: vscode.ViewColumn.Beside, preserveFocus: false });
    const line = Math.max(0, srcRef.lineNumber - 1);
    const position = new vscode.Position(line, Math.max(0, srcRef.column - 1));
    editor.selection = new vscode.Selection(position, position);
    editor.revealRange(new vscode.Range(position, position), vscode.TextEditorRevealType.InCenter);
}

class GoToSourceCodeLensProvider implements vscode.CodeLensProvider, vscode.Disposable {
    public readonly onDidChangeCodeLenses: vscode.Event<void>;
    private readonly _onDidChangeCodeLenses = new vscode.EventEmitter<void>();
    private readonly _listeners: vscode.Disposable[];
    // docUri -> last line we fired a refresh for, to avoid re-firing on no-op selection events.
    private readonly _lastLine = new Map<string, number>();

    public constructor(private readonly _index: LogMatchIndex, private readonly _output: OutputSink) {
        this.onDidChangeCodeLenses = this._onDidChangeCodeLenses.event;
        // Cursor movement changes which line the lens should appear on, so re-request lenses then.
        this._listeners = [
            vscode.window.onDidChangeTextEditorSelection((e) => this._handleSelectionChange(e.textEditor)),
            vscode.window.onDidChangeActiveTextEditor(() => this._onDidChangeCodeLenses.fire()),
        ];
    }

    public dispose(): void {
        this._listeners.forEach(d => d.dispose());
    }

    private _handleSelectionChange(editor: vscode.TextEditor): void {
        if (editor.document.languageId !== LOG_LANGUAGE_ID) {
            return;
        }
        const key = editor.document.uri.toString();
        const line = editor.selection.active.line;
        if (this._lastLine.get(key) === line) {
            return;
        }
        this._lastLine.set(key, line);
        this._onDidChangeCodeLenses.fire();
    }

    public provideCodeLenses(document: vscode.TextDocument): vscode.CodeLens[] {
        const config = getNavigationConfig();
        if (config.sourceDirs.length === 0) {
            return [];
        }

        const editor = vscode.window.visibleTextEditors.find(e => e.document.uri.toString() === document.uri.toString());
        if (!editor) {
            return [];
        }

        const line = editor.selection.active.line;
        const matches = this._index.getMatchesForRange(
            document.uri.fsPath, config.sourceDirs, config.logFormat, line, line
        );
        const mapping = matches.get(line);
        if (!mapping?.srcRef) {
            return [];
        }

        return [new vscode.CodeLens(new vscode.Range(line, 0, line, 0), {
            title: sourceRefTitle(mapping.srcRef),
            command: OPEN_SOURCE_LOCATION_COMMAND,
            arguments: [mapping.srcRef]
        })];
    }
}

async function configureSourceRootsAndFormat(output: OutputSink): Promise<void> {
    const config = vscode.workspace.getConfiguration('log2src');
    // Workspace-scoped settings require an open workspace folder; fall back to Global otherwise.
    const target = (vscode.workspace.workspaceFolders?.length ?? 0) > 0
        ? vscode.ConfigurationTarget.Workspace
        : vscode.ConfigurationTarget.Global;

    const paths = await vscode.window.showOpenDialog({
        canSelectFiles: true,
        canSelectFolders: true,
        canSelectMany: true,
        openLabel: 'Set as log2src source root(s)'
    });
    if (paths && paths.length > 0) {
        const sourceRoots = paths.map(f => f.fsPath);
        await config.update('sourceRoots', sourceRoots, target);
        output.appendLine(`log2src.sourceRoots set to: ${JSON.stringify(sourceRoots)}`);
    }

    const logFormat = await vscode.window.showInputBox({
        title: 'Log2Src: log format (regex with named captures, e.g. (?<body>.*))',
        value: config.get<string>('logFormat', ''),
        prompt: 'Leave empty to use the default log format'
    });
    if (logFormat !== undefined) {
        await config.update('logFormat', logFormat, target);
        output.appendLine(`log2src.logFormat set to: ${JSON.stringify(logFormat)}`);
    }
}

async function goToSourceAtCursor(index: LogMatchIndex): Promise<void> {
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.document.languageId !== LOG_LANGUAGE_ID) {
        return;
    }
    const config = getNavigationConfig();
    if (config.sourceDirs.length === 0) {
        vscode.window.showWarningMessage('Log2Src: set log2src.sourceRoots before using "Go to Source".');
        return;
    }
    const line = editor.selection.active.line;
    const matches = index.getMatchesForRange(
        editor.document.uri.fsPath, config.sourceDirs, config.logFormat, line, line
    );
    const mapping = matches.get(line);
    if (mapping?.srcRef) {
        await openSourceLocation(mapping.srcRef);
    } else {
        vscode.window.showInformationMessage('Log2Src: no source match found for this log line.');
    }
}

export function registerLogNavigation(context: vscode.ExtensionContext, client: Log2srcClient, output: OutputSink): void {
    const index = new LogMatchIndex(client);
    const codeLensProvider = new GoToSourceCodeLensProvider(index, output);

    context.subscriptions.push(
        vscode.languages.registerCodeLensProvider({ language: LOG_LANGUAGE_ID }, codeLensProvider),
        codeLensProvider,
        vscode.commands.registerCommand(CONFIGURE_EXTENSION, () => configureSourceRootsAndFormat(output)),
        vscode.commands.registerCommand(OPEN_SOURCE_LOCATION_COMMAND, (srcRef: SourceRef) => openSourceLocation(srcRef)),
        vscode.commands.registerCommand(GO_TO_SOURCE_AT_CURSOR_COMMAND, () => goToSourceAtCursor(index)),
        vscode.workspace.onDidCloseTextDocument((doc) => index.invalidate(doc.uri.fsPath)),
        vscode.workspace.onDidChangeTextDocument((event) => index.invalidate(event.document.uri.fsPath)),
    );
}
