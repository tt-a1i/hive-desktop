export function terminalArgument(value: string, platform = process.platform): string {
  return platform === 'win32'
    ? `'${value.replace(/['‘’]/g, character => character + character)}'`
    : `'${value.replaceAll("'", "'\"'\"'")}'`;
}

/** Copyable POSIX-shell/PowerShell argv. This formats text; it never executes it. */
export function terminalCommand(argv: readonly string[], platform = process.platform): string {
  return (platform === 'win32' ? '& ' : '') + argv.map(value => terminalArgument(value, platform)).join(' ');
}
