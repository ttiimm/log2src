/**
 * log2srcFeatures.ts implements Log2Src source/log features with VS Code.
 */

import * as vscode from 'vscode';
import * as path from 'path';

import { LogMatchIndex } from './logMatchIndex';
import { Log2srcClient, SourceRef, SourceUsage } from './log2srcClient';
import { OutputSink } from './debugAdapter';
import { SourceUsageIndex } from './sourceUsageIndex';

export const LOG_LANGUAGE_ID = 'log2src';
const CONFIGURE_EXTENSION = 'log2src.configure';
const OPEN_SOURCE_LOCATION_COMMAND = 'log2src.openSourceLocation';
const GO_TO_SOURCE_AT_CURSOR_COMMAND = 'log2src.goToSourceAtCursor';
const SHOW_SOURCE_LOGS_COMMAND = 'log2src.showSourceLogs';

interface NavigationConfig {
    sourceDirs: string[];
    logFile: string;
    logFormat?: string;
}

function getNavigationConfig(): NavigationConfig {
    const config = vscode.workspace.getConfiguration('log2src');
    return {
        sourceDirs: config.get<string[]>('sourceRoots', []),
        logFile: config.get<string>('logFile', ''),
        logFormat: config.get<string>('logFormat') || undefined,
    };
}

function resolveLogFile(logFile: string): string {
    if (path.isAbsolute(logFile)) {
        return path.normalize(logFile);
    }
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    return path.resolve(workspaceFolder?.uri.fsPath ?? process.cwd(), logFile);
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

class GoToLogsCodeLensProvider implements vscode.CodeLensProvider, vscode.Disposable {
    public readonly onDidChangeCodeLenses: vscode.Event<void>;
    private readonly _onDidChangeCodeLenses = new vscode.EventEmitter<void>();
    private readonly _listeners: vscode.Disposable[];

    public constructor(private readonly _index: SourceUsageIndex, private readonly _output: OutputSink) {
        this.onDidChangeCodeLenses = this._onDidChangeCodeLenses.event;
        this._listeners = [
            vscode.workspace.onDidChangeConfiguration(event => {
                if (event.affectsConfiguration('log2src')) {
                    this._index.invalidate();
                    this._onDidChangeCodeLenses.fire();
                }
            }),
            vscode.workspace.onDidSaveTextDocument(document => {
                const config = getNavigationConfig();
                const savedPath = path.resolve(document.uri.fsPath);
                const isLogFile = config.logFile && savedPath === resolveLogFile(config.logFile);
                const isSourceFile = config.sourceDirs.some(root => {
                    const resolvedRoot = path.resolve(root);
                    return savedPath === resolvedRoot || savedPath.startsWith(`${resolvedRoot}${path.sep}`);
                });
                if (isLogFile || isSourceFile) {
                    this._index.invalidate();
                    this._onDidChangeCodeLenses.fire();
                }
            }),
        ];
    }

    public dispose(): void {
        this._listeners.forEach(listener => listener.dispose());
        this._onDidChangeCodeLenses.dispose();
    }

    public async provideCodeLenses(document: vscode.TextDocument, token: vscode.CancellationToken): Promise<vscode.CodeLens[]> {
        const config = getNavigationConfig();
        if (document.uri.scheme !== 'file' || config.sourceDirs.length === 0 || !config.logFile) {
            return [];
        }

        const scanController = new AbortController();
        try {
            const logFile = resolveLogFile(config.logFile);
            const usages = await vscode.window.withProgress(
                { location: vscode.ProgressLocation.Window, title: 'Log2Src: Scanning log for source matches', cancellable: true },
                (_progress, progressToken) => {
                    const abort = (): void => scanController.abort();
                    const requestCancellation = token.onCancellationRequested(abort);
                    const progressCancellation = progressToken.onCancellationRequested(abort);
                    if (token.isCancellationRequested || progressToken.isCancellationRequested) {
                        abort();
                    }
                    return this._index.getUsages(config.sourceDirs, logFile, config.logFormat, scanController.signal)
                        .finally(() => {
                            requestCancellation.dispose();
                            progressCancellation.dispose();
                        });
                }
            );
            if (token.isCancellationRequested) {
                return [];
            }

            const documentPath = _normalizeFsPath(document.uri.fsPath);
            return usages
                .filter(usage => _normalizeFsPath(usage.sourcePath) === documentPath)
                .map(usage => new vscode.CodeLens(
                    new vscode.Range(Math.max(0, usage.lineNumber - 1), 0, Math.max(0, usage.lineNumber - 1), 0),
                    {
                        title: `${usage.count} matching log message${usage.count === 1 ? '' : 's'}`,
                        command: SHOW_SOURCE_LOGS_COMMAND,
                        arguments: [logFile, usage],
                    }
                ));
        } catch (error) {
            if (scanController.signal.aborted) {
                return [];
            }
            this._output.appendLine(`log2src: failed to scan ${config.logFile}: ${String(error)}`);
            return [];
        }
    }
}

function _normalizeFsPath(p: string): string {
    // canonical paths on windows can contain a \\?\ in the path, for example `\\?\D:\a\log2src\log2src\examples\basic.rs`,
    // that tell Windows it's a "verbatim" path. This should probably be normalized in log2src, but this fixes it for now.
    const resolved = path.resolve(p.replace(/^\\\\\?\\/, ''));
    return process.platform === 'win32' ? resolved.toLowerCase() : resolved;
}

async function showSourceLogs(logFile: string, usage: SourceUsage): Promise<void> {
    const selected = await vscode.window.showQuickPick(
        usage.samples.map(sample => ({
            label: `Log line ${sample.lineNumber}`,
            description: sample.text,
            lineNumber: sample.lineNumber,
        })),
        {
            title: `${usage.count} matches for ${usage.name}`,
            placeHolder: `Showing up to ${usage.samples.length} sample messages`,
            matchOnDescription: true,
        }
    );
    if (!selected) {
        return;
    }

    const document = await vscode.workspace.openTextDocument(vscode.Uri.file(logFile));
    const editor = await vscode.window.showTextDocument(document, { preserveFocus: false });
    const position = new vscode.Position(Math.max(0, selected.lineNumber - 1), 0);
    editor.selection = new vscode.Selection(position, position);
    editor.revealRange(new vscode.Range(position, position), vscode.TextEditorRevealType.InCenter);
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

    const logFiles = await vscode.window.showOpenDialog({
        canSelectFiles: true,
        canSelectFolders: false,
        canSelectMany: false,
        openLabel: 'Use as log file',
        title: 'Log2Src: Select log file for source CodeLens',
    });
    if (logFiles && logFiles.length > 0) {
        await config.update('logFile', logFiles[0].fsPath, target);
        output.appendLine(`log2src.logFile set to: ${JSON.stringify(logFiles[0].fsPath)}`);
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

export function registerLog2srcFeatures(context: vscode.ExtensionContext, client: Log2srcClient, output: OutputSink): void {
    const index = new LogMatchIndex(client);
    const codeLensProvider = new GoToSourceCodeLensProvider(index, output);
    const sourceUsageIndex = new SourceUsageIndex(client);
    const sourceCodeLensProvider = new GoToLogsCodeLensProvider(sourceUsageIndex, output);

    context.subscriptions.push(
        vscode.languages.registerCodeLensProvider({ language: LOG_LANGUAGE_ID }, codeLensProvider),
        vscode.languages.registerCodeLensProvider({ scheme: 'file' }, sourceCodeLensProvider),
        codeLensProvider,
        sourceCodeLensProvider,
        vscode.commands.registerCommand(CONFIGURE_EXTENSION, () => configureSourceRootsAndFormat(output)),
        vscode.commands.registerCommand(OPEN_SOURCE_LOCATION_COMMAND, (srcRef: SourceRef) => openSourceLocation(srcRef)),
        vscode.commands.registerCommand(GO_TO_SOURCE_AT_CURSOR_COMMAND, () => goToSourceAtCursor(index)),
        vscode.commands.registerCommand(SHOW_SOURCE_LOGS_COMMAND, (logFile: string, usage: SourceUsage) => showSourceLogs(logFile, usage)),
        vscode.workspace.onDidCloseTextDocument((doc) => index.invalidate(doc.uri.fsPath)),
        vscode.workspace.onDidChangeTextDocument((event) => index.invalidate(event.document.uri.fsPath)),
    );
}
