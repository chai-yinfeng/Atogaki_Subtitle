import test from 'node:test';
import assert from 'node:assert/strict';
import { createMediaPlayer } from '../src/media-player.ts';
class Media extends EventTarget {
 constructor(kind) { super();this.kind=kind;this.currentTime=0;this.duration=120;this.paused=true;this.volume=1;this.muted=false;this.defaultPlaybackRate=1;this.playbackRate=1;this.error=null;this.loads=0;this.parent=null; }
 set src(value) { this._src=value;this.playbackRate=this.defaultPlaybackRate; }
 get src() { return this._src; }
 emit(name) { this.dispatchEvent(new Event(name)); }
 play() { this.paused=false;this.emit('play');return this.playResult ?? Promise.resolve(); }
 pause() { if(!this.paused){this.paused=true;this.emit('pause');} }
 removeAttribute(name) { if(name==='src')this.src=''; }
 load() { this.loads++;this.currentTime=0; }
 remove() { if(this.parent){this.parent.children=this.parent.children.filter(e=>e!==this);this.parent=null;} }
}
const host=()=>({children:[],replaceChildren(...children){for(const child of this.children)child.parent=null;this.children=children;for(const child of children)child.parent=this;}});
function harness() {
 const elements=[],positions=[],calls=[],frames=new Map();let id=0,current='a',animate=true;
 const player=createMediaPlayer({currentJob:job=>job===current,resolve:p=>{calls.push(['resolve',p]);return 'asset:'+p;},isAudio:p=>p.endsWith('.wav'),time:ms=>String(ms),position:ms=>positions.push(ms),progress:()=>calls.push(['progress']),changed:()=>calls.push(['changed']),persist:()=>calls.push(['persist']),frameEnabled:()=>animate},{create:kind=>{const e=new Media(kind);elements.push(e);return e;},requestFrame:f=>{frames.set(++id,f);return id;},cancelFrame:id=>frames.delete(id)});
 const source={jobId:'a',inputPath:'/movie.mp4',primaryPath:'/movie.mp4',fallbackPath:'/audio.wav',host:host(),message:{textContent:''},resumeMs:70000,playbackRate:1.25};
 return {player,elements,positions,calls,frames,source,setJob:job=>current=job,setAnimate:value=>animate=value,tick(){const [id,callback]=frames.entries().next().value;frames.delete(id);callback(0);}};
}
test('fallback preserves playback, position, volume, mute and rate, and old elements lose all listeners',async()=>{
 const h=harness();h.player.mount(h.source);const video=h.player.media;
 assert.equal(video.kind,'video');video.emit('loadedmetadata');assert.equal(video.currentTime,70);video.emit('seeked');
 video.currentTime=81.5;video.volume=.4;video.muted=true;video.playbackRate=1.5;await video.play();
 video.error={code:3};video.pause();video.emit('error');const audio=h.player.media;
 assert.notEqual(audio,video);assert.equal(audio.kind,'audio');assert.equal(video.src,'');assert.equal(video.loads,1);
 audio.emit('loadedmetadata');assert.equal(audio.currentTime,81.5);assert.equal(audio.paused,false);assert.equal(audio.volume,.4);assert.equal(audio.muted,true);assert.equal(audio.playbackRate,1.5);
 const positions=h.positions.length,calls=h.calls.length;for(const event of ['error','timeupdate','loadedmetadata','play','pause'])video.emit(event);
 assert.equal(h.positions.length,positions);assert.equal(h.calls.length,calls);assert.equal(h.elements.length,2);
 audio.error={code:3};audio.emit('error');assert.equal(h.elements.length,2);assert.equal(audio.paused,true);assert.match(h.source.message.textContent,/媒体加载失败/);assert.equal(h.frames.size,0);
 h.player.dispose();
});
test('metadata failure uses requested resume, and direct loopback URLs do not pass through asset conversion',()=>{
 const h=harness();h.source.primaryPath='http://127.0.0.1:1234/media/abc-def';h.player.mount(h.source);
 assert.equal(h.player.media.src,h.source.primaryPath);assert.equal(h.calls.some(c=>c[0]==='resolve'),false);
 h.player.media.emit('error');const audio=h.player.media;audio.duration=60;audio.emit('loadedmetadata');assert.equal(audio.currentTime,59.75);
 h.player.dispose();
});
test('stop snapshots the old position before unloading, and cannot remove a newer player while awaiting persistence',async()=>{
 const h=harness();h.player.mount(h.source);const old=h.player.media;old.currentTime=42;await old.play();
 let finish;const stop=h.player.stop(async element=>{assert.equal(element.currentTime,42);await new Promise(r=>finish=r);});
 assert.equal(h.player.media,null);assert.equal(h.frames.size,0);assert.equal(old.currentTime,42);
 h.player.mount({...h.source,primaryPath:'/new.mp4'});const next=h.player.media;
 finish();await stop;assert.equal(h.player.media,next);assert.deepEqual(h.source.host.children,[next]);assert.equal(old.currentTime,0);
 h.player.dispose();
});
test('frames are single, job scoped and stopped on pause, unmount or dispose',async()=>{
 const h=harness();h.player.mount(h.source);const element=h.player.media;await element.play();h.player.refreshAnimation();assert.equal(h.frames.size,1);
 element.currentTime=9;h.tick();assert.equal(h.positions.at(-1),9000);assert.equal(h.frames.size,1);
 h.setJob('b');const count=h.positions.length;h.tick();assert.equal(h.positions.length,count);assert.equal(h.frames.size,0);
 h.setJob('a');h.player.refreshAnimation();assert.equal(h.frames.size,1);element.pause();assert.equal(h.frames.size,0);
 await element.play();h.player.dispose();assert.equal(h.frames.size,0);assert.equal(h.player.media,null);
 element.emit('loadedmetadata');assert.equal(h.positions.length,count);
 h.player.mount(h.source);assert.equal(h.player.media,null);
});
test('a rejected play promise from an old element cannot alter the next task message',async()=>{
 const h=harness();h.player.mount(h.source);const video=h.player.media;await video.play();video.error={code:3};video.emit('error');
 const old=h.player.media;let reject;old.playResult=new Promise((_,r)=>reject=r);old.emit('loadedmetadata');
 h.player.mount({...h.source,primaryPath:'/next.mp4',fallbackPath:null});const message=h.source.message.textContent;
 reject(new Error('old play rejected'));await Promise.resolve();await Promise.resolve();
 assert.equal(h.source.message.textContent,message);assert.equal(h.player.media.src,'asset:/next.mp4');h.player.dispose();
});
test('rapid seeks serialize native assignments and coalesce intermediate targets',()=>{
 const h=harness();h.source.resumeMs=0;h.player.mount(h.source);const media=h.player.media;media.emit('loadedmetadata');
 h.player.seek(2000);assert.equal(media.currentTime,2);media.seeking=true;
 h.player.seek(5000);h.player.seek(7000);assert.equal(media.currentTime,2);assert.equal(h.player.positionMs,7000);
 media.emit('timeupdate');assert.notEqual(h.positions.at(-1),2000);
 media.seeking=false;media.emit('seeked');assert.equal(media.currentTime,7);
 media.emit('seeked');assert.equal(h.positions.at(-1),7000);assert.equal(h.player.positionMs,7000);
 h.player.seek(10000);h.player.seek(200000);assert.equal(h.player.positionMs,120000);h.player.seek(NaN);assert.equal(h.player.positionMs,120000);
 h.player.seek(-100);assert.equal(h.player.positionMs,0);media.emit('seeked');assert.equal(media.currentTime,0);media.emit('seeked');h.player.dispose();
});
test('pre-metadata seek supersedes resume and fallback carries the latest queued intent',async()=>{
 const h=harness();h.player.mount(h.source);const media=h.player.media;
 h.player.seek(4000);assert.equal(media.currentTime,0);media.emit('loadedmetadata');assert.equal(media.currentTime,4);
 h.player.seek(12000);media.emit('error');const fallback=h.player.media;fallback.emit('loadedmetadata');assert.equal(fallback.currentTime,12);
 fallback.emit('seeked');h.player.seek(20000);h.player.seek(30000);
 await h.player.stop(async (element,position)=>{assert.equal(element,fallback);assert.equal(position,30000)});media.emit('seeked');fallback.emit('seeked');assert.equal(h.player.media,null);h.player.dispose();
});
test('native-control seeks queue app seeks; obsolete elements cannot drain into the next task',()=>{
 const h=harness();h.source.resumeMs=0;h.player.mount(h.source);const old=h.player.media;old.emit('loadedmetadata');old.seeking=true;
 h.player.seek(9000);assert.equal(old.currentTime,0);old.seeking=false;old.emit('seeked');assert.equal(old.currentTime,9);
 h.player.seek(18000);h.player.mount({...h.source,primaryPath:'/next.mp4'});const next=h.player.media;next.emit('loadedmetadata');old.emit('seeked');assert.equal(next.currentTime,0);h.player.dispose();
});
