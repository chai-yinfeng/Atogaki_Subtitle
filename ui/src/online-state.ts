export type OnlineStatus = { sessionId: string | null; phase: string; error: string | null };
export type LiveEvent = { kind: string; text?: string; source_text?: string; segment_id?: string; group_id?: string; start_s?: number; end_s?: number; after_audio_end_s?: number; stale_source?: boolean; provisional?: boolean };
export type CaptionEvent = { sessionId: string; sequence: number; event: LiveEvent };
export type OnlineSnapshot = { status: OnlineStatus; events: CaptionEvent[] };
export const phaseLabel: Record<string,string> = { idle:"未开始", loading:"正在加载模型", requesting_audio:"等待系统音频权限", running:"正在接收系统音频", stopping:"正在停止采集", draining:"正在完成最后一段", stopped:"已停止", failed:"会话失败" };
export const activePhase = (phase:string) => ["loading","requesting_audio","running","stopping","draining"].includes(phase);

/** The source and MT tracks have different units and finality. Keep both. */
export class OnlineCaptions {
  status: OnlineStatus = {sessionId:null,phase:"idle",error:null};
  private events = new Map<number, CaptionEvent>();
  snapshot(snapshot:OnlineSnapshot):void {
    if(snapshot.status.sessionId !== this.status.sessionId) this.events.clear();
    this.status=snapshot.status;
    for(const e of snapshot.events) this.accept(e);
  }
  accept(event:CaptionEvent):void {
    if(event.sessionId!==this.status.sessionId) return;
    this.events.set(event.sequence,event);
    while(this.events.size>256) this.events.delete(Math.min(...this.events.keys()));
  }
  tracks(): {source:LiveEvent|null; translation:LiveEvent|null; history:LiveEvent[]} {
    const sources=new Map<string,LiveEvent>(), groups=new Map<string,LiveEvent>();
    for(const {event:e} of [...this.events.values()].sort((a,b)=>a.sequence-b.sequence)) {
      if(e.kind.startsWith("source_") && e.segment_id) sources.set(e.segment_id,e);
      if(e.kind.startsWith("translation_") && e.group_id && e.text!==undefined) groups.set(e.group_id,e);
    }
    const byTime=(a:LiveEvent,b:LiveEvent)=>(a.start_s??0)-(b.start_s??0);
    const source=[...sources.values()].sort(byTime).at(-1)??null;
    const translations=[...groups.values()].sort(byTime);
    return { source, translation:translations.at(-1)??null, history:translations.filter(e=>e.kind==="translation_final").slice(-3) };
  }
}
