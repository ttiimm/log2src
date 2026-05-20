import * as vscode from 'vscode';
import { EditorEffects, OutputSink } from './debugAdapter';
import * as path from 'path';


export class VscodeEditorEffects implements EditorEffects {
    private readonly _highlightDecoration: vscode.TextEditorDecorationType;
    private readonly _output: OutputSink;

    constructor(output: OutputSink) {
        const focusColor = new vscode.ThemeColor('editor.focusedStackFrameHighlightBackground');
        this._highlightDecoration = vscode.window.createTextEditorDecorationType({
            backgroundColor: focusColor
        });
        this._output = output;
    }

    public openAndFocus(log: string, line: number): void {
        const editors = this.findEditors(log);
        if (editors.length >= 1) {
            this.focusEditor(editors[0], line);
        } else {
            Promise.resolve(vscode.workspace.openTextDocument(log))
                .then(doc => {
                    return vscode.window.showTextDocument(doc, {
                        viewColumn: vscode.ViewColumn.Beside,
                        preserveFocus: false
                    });
                })
                .then(editor => {
                    this.focusEditor(editor, line);
                    return editor;
                })
                .catch(error => {
                    const message = `Failed to open log file: ${error.message}`;
                    this._output.appendLine(message);
                    console.error(message);
                });
        }
    }

    public highlightLine(log: string, line: number): void {
        const editor = this.findEditors(log);
        if (editor.length > 0) {
            this.focusEditor(editor[0], line);
        }
    }

    public clearHighlights(): void {
        vscode.window.visibleTextEditors.forEach((editor) => editor.setDecorations(this._highlightDecoration, []));
    }

    private findEditors(log: string): vscode.TextEditor[] {
        const target = path.resolve(log);
        return vscode.window.visibleTextEditors.filter((editor) => path.resolve(editor.document.fileName) === target);
    }

    private focusEditor(editor: vscode.TextEditor, line: number): void {
        const start = Math.max(0, line - 1);
        let range = new vscode.Range(
            new vscode.Position(start, 0),
            new vscode.Position(start, Number.MAX_VALUE)
        );
        editor.setDecorations(this._highlightDecoration, [range]);
        editor.revealRange(
            range,
            vscode.TextEditorRevealType.InCenter
        );
    }
}
