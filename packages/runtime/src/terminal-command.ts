/** Copyable POSIX-shell/PowerShell argv. This formats text; it never executes it. */
export function terminalCommand(argv:readonly string[],platform=process.platform):string {
  const quote=platform==='win32'
    ? (value:string)=>`'${value.replaceAll("'","''")}'`
    : (value:string)=>`'${value.replaceAll("'","'\"'\"'")}'`;
  return (platform==='win32'?'& ':'')+argv.map(quote).join(' ');
}
