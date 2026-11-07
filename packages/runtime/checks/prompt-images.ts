/** Image byte contracts and durable transport spool, no vendor requests. */
import assert from 'node:assert/strict';
import { mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { promptImages, imageMessage, MAX_IMAGE_BASE64, MAX_TOTAL_IMAGE_BASE64 } from '../src/prompt-images.js';
import { DeliveryQueue } from '../src/delivery-queue.js';
import { normalizeCodexItem } from '../src/agents/codex-history.js';
import { projectClaudeMessage } from '../src/agents/claude-history.js';
const image={type:'image' as const,mimeType:'image/png',data:'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC'};
assert.deepEqual(promptImages([image]),[image]);
for(const [mimeType,bytes] of [
 ['image/jpeg',Buffer.from([255,216,255,224,0,16])],
 ['image/gif',Buffer.from('GIF89a')],
 ['image/webp',Buffer.from('RIFF0000WEBP')],
] as const){const header={type:'image',mimeType,data:bytes.toString('base64')};assert.deepEqual(promptImages([header]),[header]);}

assert.deepEqual(imageMessage('核验',[image]),[{type:'text',text:'核验'},image]);
assert.deepEqual(imageMessage('',[image]),[image]);
assert.equal(imageMessage('text',[]),'text');
for(const invalid of [null,{},[{}],[{...image,type:'file'}],[{...image,mimeType:'image/jpeg'}],[{...image,data:'!!'}],[{...image,data:image.data+'\n'}],Array(11).fill(image)]) assert.throws(()=>promptImages(invalid));
assert.throws(()=>promptImages([{...image,data:'A'.repeat(MAX_IMAGE_BASE64+4)}]),RangeError);
const large=Buffer.alloc(3_000_000);Buffer.from(image.data,'base64').copy(large);
assert.ok(large.toString('base64').length*6>MAX_TOTAL_IMAGE_BASE64);
assert.throws(()=>promptImages(Array(6).fill({...image,data:large.toString('base64')})),RangeError);
const directory=await mkdtemp(join(tmpdir(),'hive-image-spool-'));
try{
 const fail=(error:Error)=>{throw error;};
 const queue=new DeliveryQueue({directory,sessionId:'images',startPaused:true,completedIds:()=>[],execute:async()=>{throw Error('Paused queue must not execute');},onError:fail});
 const mutable={...image};await queue.enqueue('中文图片',true,undefined,[mutable]);mutable.data='changed after enqueue';
 assert.deepEqual(queue.pending[0]!.images,[image]);
 (queue.pending[0]!.images![0] as {data:string}).data='changed snapshot';assert.deepEqual(queue.pending[0]!.images,[image]);
 const [filename]=await readdir(directory);const path=join(directory,filename!);const stored=JSON.parse(await readFile(path,'utf8'));
 assert.deepEqual(stored[0].images,[image]);
 let completed!:()=>void;const done=new Promise<void>(r=>{completed=r;});let received:unknown;
 const restored=new DeliveryQueue({directory,sessionId:'images',startPaused:true,completedIds:()=>[],execute:async item=>{received=item;completed();},onError:fail});
 assert.equal(restored.pendingCount,1);restored.resume();await done;await restored.idle();
 assert.deepEqual((received as {images:unknown}).images,[image]);assert.equal((received as {text:string}).text,'中文图片');assert.deepEqual(JSON.parse(await readFile(path,'utf8')),[]);
 await assert.rejects(queue.enqueue('invalid',true,undefined,[{...image,data:'broken'}]));
 await assert.rejects(queue.enqueue('context',false,undefined,[image]));
 await writeFile(path,JSON.stringify([{...stored[0],images:[{...image,data:'broken'}]}]));
 assert.throws(()=>DeliveryQueue.persistedCount(directory,'images'));
 await writeFile(path,JSON.stringify([{...stored[0],triggerTurn:false}]));assert.throws(()=>DeliveryQueue.persistedCount(directory,'images'));
 const native={type:'userMessage',id:'input',content:[{type:'text',text:'原图'},{type:'image',url:`data:${image.mimeType};base64,${image.data}`}]};
 const normalized=normalizeCodexItem(native);assert.deepEqual(normalized.content,[{type:'text',text:'原图'},image]);assert.deepEqual(normalized.rawBackendItem,native);
 const source={type:'user',uuid:'image',session_id:'fixture',parent_tool_use_id:null,message:{role:'user',content:[{type:'image',source:{type:'base64',media_type:image.mimeType,data:image.data}}]}};
 const metadata={context:[],deliveries:[],completed:[]};
 const projected=projectClaudeMessage(source as never,metadata as never)[0]!;assert.deepEqual(projected.message.content,[image]);assert.deepEqual(projected.message.rawBackendItem,source);
 console.log('PROMPT_IMAGES_OK: bounded canonical image payloads, immutable durable queue and recovery, corrupt spool rejection, Codex/Claude raw provenance and normalized images. No model requests.');
}finally{await rm(directory,{recursive:true,force:true});}
