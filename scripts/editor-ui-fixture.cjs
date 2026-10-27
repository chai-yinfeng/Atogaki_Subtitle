// Synthetic IPC and media shared by UI regressions and WebKit measurements.
module.exports.installEditorFixture = async function(page, cueCount = 300, durationMs = cueCount * 3000) {
// Generated silence tests playback without user media, models or network providers.
const wav=Buffer.alloc(44+16000*20);wav.write('RIFF');wav.writeUInt32LE(wav.length-8,4);wav.write('WAVEfmt ',8);wav.writeUInt32LE(16,16);wav.writeUInt16LE(1,20);wav.writeUInt16LE(1,22);wav.writeUInt32LE(8000,24);wav.writeUInt32LE(16000,28);wav.writeUInt16LE(2,32);wav.writeUInt16LE(16,34);wav.write('data',36);wav.writeUInt32LE(wav.length-44,40);
await page.addInitScript(({audioUrl,cueCount,durationMs})=>{
  window.__qaAudio=audioUrl;
  const cues=Array.from({length:cueCount},(_,i)=>({id:`cue-${i}`,job_id:'editor-test',segment_index:i,start_ms:Math.floor(i*durationMs/cueCount),end_ms:Math.floor((i+1)*durationMs/cueCount)-Math.min(200,Math.floor(durationMs/cueCount/4)),source_text:`第 ${i+1} 句：今日は音楽と日常について話します。`,translated_text:i%3?'今天聊聊音乐与日常。':null,translation_stale:i%7===0,source_edited:false,translation_edited:false,timing_edited:false}));
  const job={job_id:'editor-test',display_name:'编辑器隔离测试 · 音乐与日常',input_path:'/test/program.wav',storage_dir:'/test',status:'done',message:'完成',source_language:'ja',target_language:'zh-Hans',created_at_unix:1,updated_at_unix:1,segment_count:cues.length,translated_segment_count:Math.floor(cueCount*2/3),stale_translation_count:Math.ceil(cueCount/7),translation_status:'stale'};
  window.__test={cues,job,calls:[],fail:false,delay:0};
  // Synthetic backend token history; never restore from a client-provided snapshot.
  const histories=new Map(),revisions=new Map();
  const rows=id=>id==='job-b'?window.__test.bCues:cues;
  const revision=id=>revisions.get(id)??0;
  const latest=id=>{const entry=histories.get(id)?.at(-1);return entry?.descriptor.expected_revision===revision(id)?structuredClone(entry.descriptor):null;};
  window.__test.recordStructure=(before,label,id='editor-test')=>{
    const rev=revision(id)+1;revisions.set(id,rev);
    const history=histories.get(id)??[];
    if(window.__test.structureOversized){
      histories.delete(id);
      return {segments:structuredClone(rows(id)),workspace_revision:rev,translation_context_stale_ids:[],undo:{latest:null,discarded_entries:history.length,oversized:true},warnings:[]};
    }

    const descriptor={token:crypto.randomUUID(),label,expected_revision:rev};
    history.push({before:structuredClone(before),descriptor});histories.set(id,history);
    return {segments:structuredClone(rows(id)),workspace_revision:rev,translation_context_stale_ids:[],undo:{latest:structuredClone(descriptor),discarded_entries:0,oversized:false},warnings:window.__test.structureWarnings??[]};
  };
  window.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},transformCallback:()=>1,unregisterCallback:()=>{},convertFileSrc:path=>path==='/fixture.wav'?window.__qaAudio:path,invoke:async(cmd,args={})=>{
    window.__test.calls.push({cmd,args});
    if(cmd==='get_job_detail')return {job:{...job},segments:cues.map(cue=>({...cue})),translation_runs:[],translation_context_stale_ids:[],playback_path:'/fixture.wav',audio_fallback_path:null};
    if(cmd==='list_jobs' || cmd==='list_job_summaries')return [job];
    if(cmd==='desktop_settings')return {translationProviderId:'none',translationStyleInstruction:'',modelsDirectory:'/test/models',networkProxyMode:'environment',credentialStore:'test',needsOnboarding:false};
    if(cmd==='translation_status')return {provider_id:'none',provider:'测试翻译',configured:false,model:null,endpoint_kind:'none',configuration_hint:'测试'};
    if(cmd==='get_waveform_window'){const {startMs,endMs,pointCount}=args.request;return {start_ms:startMs,end_ms:endMs,duration_ms:durationMs,peaks:Array.from({length:pointCount},(_,i)=>({min:-Math.abs(Math.sin(i/8))*.7,max:Math.abs(Math.sin(i/8))*.7}))};}
    if(cmd==='list_asr_quality_signals')return [{id:'signal',signal_kind:'boundary_repetition',start_ms:3000,end_ms:5800,severity:'warning',message:'测试提示',latest_decision:null}];
    if(cmd==='update_subtitle') { await new Promise(resolve=>setTimeout(resolve,window.__test.delay)); if(window.__test.fail)throw Error('disk full'); const r=args.request,c=cues.find(c=>c.id===r.segmentId);revisions.set(r.jobId,revision(r.jobId)+1);Object.assign(c,{source_text:r.sourceText,translated_text:r.translatedText,start_ms:r.startMs,end_ms:r.endMs,source_edited:true});return {...c}; }
    if(cmd==='get_translation_context_state')return {workspace_revision:revision(args.jobId),translation_context_stale_ids:[]};
    if(cmd==='get_structure_edit_state')return {segments:structuredClone(rows(args.jobId)),workspace_revision:revision(args.jobId),latest:latest(args.jobId)};
    if(cmd==='save_subtitle_timing_recorded'){const before=structuredClone(cues);cues.splice(0,cues.length,...args.request.afterSegments);return window.__test.recordStructure(before,'调整时间',args.request.jobId);}
    if(cmd==='undo_subtitle_structure'){
      const id=args.request.jobId,history=histories.get(id),entry=history?.at(-1);
      if(!entry||entry.descriptor.token!==args.request.token||entry.descriptor.expected_revision!==revision(id))throw Error('fixture stale undo token');
      history.pop();rows(id).splice(0,rows(id).length,...structuredClone(entry.before));
      const rev=revision(id)+1;revisions.set(id,rev);if(history.length)history.at(-1).descriptor.expected_revision=rev;
      return {segments:structuredClone(rows(id)),workspace_revision:rev,translation_context_stale_ids:[],latest:latest(id),discarded_entries:0};
    }
    if(cmd==='data_directory')return '/test/isolated';
    if(cmd==='theme_artwork_settings')return null;
    if(cmd.startsWith('plugin:'))return 1;
    if(['translation_profiles','model_catalog','model_download_states','dictionary_catalog','dictionary_download_states','dictionary_credential_statuses','list_glossaries','task_presets','list_video_renders','list_asr_runs'].includes(cmd))return [];
    return [];
  }};
  window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:()=>{}};
}, {audioUrl:'data:audio/wav;base64,'+wav.toString('base64'),cueCount,durationMs});
};
