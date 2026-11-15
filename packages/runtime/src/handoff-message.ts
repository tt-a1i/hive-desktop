import type {ChatBackend} from './chat-index.js';
/** Identities come from the backend's persisted projection, never display text or list position. */
export function handoffMessage(backend:ChatBackend,message:unknown,persistedMessageId?:string):unknown{
 if(!message||typeof message!=='object'||Array.isArray(message))return message;
 const m=message as Record<string,unknown>,source=m.source&&typeof m.source==='object'?m.source as Record<string,unknown>:undefined;
 const id=backend==='codex'?m.itemId:backend==='pi'||backend==='pi-rpc'?source?.entryId:backend==='claude'?source?.nativeUuid:persistedMessageId;
 if(typeof id!=='string'||!id||m.applicationEvent==='native-user-input')return message;
 return {...m,handoffItemId:id};
}
