const vscode = require('vscode');

const TERMINAL_NAME = 'pick';

function activate(context) {
  context.subscriptions.push(
    vscode.commands.registerCommand('pick.openTerminal', () => {
      const existing = vscode.window.terminals.find(t => t.name === TERMINAL_NAME);
      if (existing) {
        existing.show();
        return;
      }
      createTerminal();
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('pick.openNewTerminal', () => {
      createTerminal();
    })
  );
}

function createTerminal() {
  const terminal = vscode.window.createTerminal({
    name: TERMINAL_NAME,
    location: {
      viewColumn: vscode.ViewColumn.Beside,
      preserveFocus: false,
    },
  });
  terminal.show();
  terminal.sendText('pick');
}

function deactivate() {}

module.exports = { activate, deactivate };
