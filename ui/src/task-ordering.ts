type Options={busy():boolean;save():Promise<void>;message(text:string):void};
/** Only the handle starts a drag. A floating copy follows the pointer; the gap previews insertion. */
export function mountTaskOrdering(list:HTMLElement,scrollHost:HTMLElement,options:Options) {
 const events=new AbortController();
 const cards=()=>[...list.querySelectorAll<HTMLElement>('[data-job-card]')];
 let drag:{card:HTMLElement;before:HTMLElement[];startX:number;startY:number;offsetX:number;offsetY:number;width:number;height:number;pointer:number;x:number;y:number;ghost:HTMLElement|null;gap:HTMLElement|null}|null=null;
 let frame:number|null=null;
 const position=()=>{
  if(!drag?.ghost||!drag.gap)return;
  drag.ghost.style.left=`${drag.x-drag.offsetX}px`;drag.ghost.style.top=`${drag.y-drag.offsetY}px`;
  const next=cards().find(card=>card!==drag!.card&&drag!.y<card.getBoundingClientRect().top+card.offsetHeight/2);
  list.insertBefore(drag.gap,next??null);
 };
 const finish=(commit:boolean)=>{
  if(!drag)return;
  const current=drag;drag=null;if(frame!==null)cancelAnimationFrame(frame);frame=null;
  current.ghost?.remove();
  if(commit&&current.gap)current.gap.replaceWith(current.card);
  else {current.gap?.remove();current.before.forEach(card=>list.append(card));}
  current.card.hidden=false;
  current.card.querySelector<HTMLElement>('.job-drag-handle')?.focus({preventScroll:true});
  if(commit&&current.ghost)void options.save();else options.message('');
 };
 const scroll=()=>{
  frame=null;if(!drag?.ghost)return;
  const bounds=scrollHost.getBoundingClientRect(),edge=48;
  const delta=drag.y<bounds.top+edge?-12:drag.y>bounds.bottom-edge?12:0;
  if(delta){scrollHost.scrollTop+=delta;position();}
  frame=requestAnimationFrame(scroll);
 };
 list.addEventListener('pointerdown',event=>{
  const handle=(event.target as Element).closest<HTMLElement>('.job-drag-handle');
  if(event.button!==0||!handle||options.busy())return;
  const card=handle.closest<HTMLElement>('[data-job-card]');if(!card)return;
  const bounds=card.getBoundingClientRect();event.preventDefault();
  drag={card,before:cards(),startX:event.clientX,startY:event.clientY,offsetX:event.clientX-bounds.left,offsetY:event.clientY-bounds.top,width:bounds.width,height:bounds.height,pointer:event.pointerId,x:event.clientX,y:event.clientY,ghost:null,gap:null};
 },{signal:events.signal});
 window.addEventListener('pointermove',event=>{
  if(!drag||drag.pointer!==event.pointerId)return;
  if(!(event.buttons&1)){finish(false);return;}
  drag.x=event.clientX;drag.y=event.clientY;
  if(!drag.ghost&&Math.hypot(drag.x-drag.startX,drag.y-drag.startY)<5)return;
  event.preventDefault();
  if(!drag.ghost){
   const ghost=drag.card.cloneNode(true) as HTMLElement;ghost.removeAttribute('data-job-card');ghost.removeAttribute('id');ghost.querySelectorAll('[id]').forEach(node=>node.removeAttribute('id'));ghost.classList.add('job-drag-preview');ghost.inert=true;ghost.setAttribute('aria-hidden','true');ghost.style.width=`${drag.width}px`;scrollHost.append(ghost);
   const gap=document.createElement('div');gap.className='job-drop-placeholder';gap.style.height=`${drag.height}px`;gap.setAttribute('aria-hidden','true');list.insertBefore(gap,drag.card);drag.card.hidden=true;
   drag.ghost=ghost;drag.gap=gap;options.message('松开鼠标放置；Escape 取消。');frame=requestAnimationFrame(scroll);
  }
  position();
 },{signal:events.signal,passive:false});
 window.addEventListener('pointerup',event=>{if(drag?.pointer===event.pointerId)finish(true);},{signal:events.signal});
 window.addEventListener('pointercancel',()=>finish(false),{signal:events.signal});
 window.addEventListener('blur',()=>finish(false),{signal:events.signal});
 window.addEventListener('keydown',event=>{if(drag&&event.key==='Escape'){event.preventDefault();event.stopImmediatePropagation();finish(false);}},{capture:true,signal:events.signal});
 scrollHost.addEventListener('close',()=>finish(false),{signal:events.signal});
 list.addEventListener('keydown',event=>{
  const handle=(event.target as Element).closest<HTMLElement>('.job-drag-handle');
  if(!handle||options.busy()||!['ArrowUp','ArrowDown'].includes(event.key))return;
  const card=handle.closest<HTMLElement>('[data-job-card]')!,sibling=event.key==='ArrowUp'?card.previousElementSibling:card.nextElementSibling;
  if(!(sibling instanceof HTMLElement))return;
  event.preventDefault();if(event.key==='ArrowUp')list.insertBefore(card,sibling);else list.insertBefore(sibling,card);handle.focus();void options.save();
 },{signal:events.signal});
 return {get active(){return drag!==null;},dispose(){finish(false);events.abort();}};
}
