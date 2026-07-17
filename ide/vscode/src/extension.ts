// src/extension.ts
import * as vscode from 'vscode';
import * as child_process from 'child_process';
import * as path from 'path';
import * as fs from 'fs';

export function activate(context: vscode.ExtensionContext) {
    console.log('🌹 Roze Language extension activated!');

    // Register build command
    let buildCommand = vscode.commands.registerCommand('roze.build', async () => {
        const terminal = vscode.window.createTerminal('Roze Build');
        terminal.show();
        terminal.sendText('roze-pkg build');
    });

    // Register run command
    let runCommand = vscode.commands.registerCommand('roze.run', async () => {
        const terminal = vscode.window.createTerminal('Roze Run');
        terminal.show();
        terminal.sendText('roze-pkg run');
    });

    // Register new project command
    let newCommand = vscode.commands.registerCommand('roze.new', async () => {
        const name = await vscode.window.showInputBox({
            prompt: 'Enter project name',
            placeHolder: 'my_project'
        });

        if (name) {
            const workspaceFolders = vscode.workspace.workspaceFolders;
            if (workspaceFolders) {
                const folderPath = vscode.Uri.joinPath(workspaceFolders[0].uri, name);
                const terminal = vscode.window.createTerminal('Roze New');
                terminal.show();
                terminal.sendText(`roze-pkg new ${name}`);
            } else {
                vscode.window.showErrorMessage('Please open a workspace folder first');
            }
        }
    });

    // Register format on save
    let formatOnSave = vscode.workspace.onDidSaveTextDocument(async (document) => {
        if (document.languageId === 'roze') {
            const config = vscode.workspace.getConfiguration('roze');
            if (config.get('formatOnSave')) {
                // Simple formatting: trim trailing whitespace
                const text = document.getText();
                const formatted = text.split('\n').map(line => line.replace(/\s+$/, '')).join('\n');
                if (text !== formatted) {
                    const edit = new vscode.WorkspaceEdit();
                    const fullRange = new vscode.Range(
                        document.positionAt(0),
                        document.positionAt(text.length)
                    );
                    edit.replace(document.uri, fullRange, formatted);
                    await vscode.workspace.applyEdit(edit);
                    await document.save();
                }
            }
        }
    });

    // Register syntax errors
    let diagnosticCollection = vscode.languages.createDiagnosticCollection('roze');

    // Simple linter - checks for common issues
    let linter = vscode.workspace.onDidSaveTextDocument((document) => {
        if (document.languageId === 'roze') {
            const diagnostics: vscode.Diagnostic[] = [];
            const text = document.getText();

            // Check for common issues
            const lines = text.split('\n');
            lines.forEach((line, index) => {
                // Check for missing semicolons (simple check)
                if (line.trim() && !line.trim().endsWith('{') && !line.trim().endsWith('}') &&
                    !line.trim().endsWith(';') && !line.trim().startsWith('//') &&
                    !line.trim().startsWith('/*') && !line.trim().endsWith('*/')) {
                    if (line.trim().match(/^(let|println|return|if|for|while)/)) {
                        const range = new vscode.Range(
                            new vscode.Position(index, line.length - 1),
                            new vscode.Position(index, line.length)
                        );
                        const diagnostic = new vscode.Diagnostic(
                            range,
                            'Missing semicolon?',
                            vscode.DiagnosticSeverity.Warning
                        );
                        diagnostics.push(diagnostic);
                    }
                }
            });

            diagnosticCollection.set(document.uri, diagnostics);
        }
    });

    // Status bar item
    let statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right);
    statusBar.text = '🌹 Roze';
    statusBar.tooltip = 'Roze Language';
    statusBar.command = 'roze.build';
    statusBar.show();

    context.subscriptions.push(
        buildCommand,
        runCommand,
        newCommand,
        formatOnSave,
        linter,
        diagnosticCollection,
        statusBar
    );
}

export function deactivate() {
    console.log('🌹 Roze Language extension deactivated!');
}