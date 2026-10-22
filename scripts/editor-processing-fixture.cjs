module.exports.installProcessingFixture = async page => {
 await require('./editor-ui-fixture.cjs').installEditorFixture(page,20,60000);
 await page.addInitScript(()=>{
  const test=window.__test,base=window.__TAURI_INTERNALS__.invoke;
  const bJob={...test.job,job_id:'job-b',display_name:'独立任务 B',storage_dir:'/test/b'};
  const bCues=test.cues.map((c,i)=>({...c,id:`b-${i}`,job_id:'job-b',source_text:`任务 B 第 ${i+1} 段`}));
  Object.assign(test,{bJob,bCues,pending:[],held:{},previews:{},confirmConflict:false,processingSerial:0});
  const segments=id=>id==='job-b'?bCues:test.cues;
  const defer=(kind,jobId,value)=>test.held[kind]?new Promise((resolve,reject)=>test.pending.push({kind,jobId,resolve:()=>resolve(value),reject})):Promise.resolve(value);
  const preview=jobId=>{
   const first=segments(jobId)[0];
   const value={preview_id:`run-${++test.processingSerial}`,comparison_id:`comparison-${test.processingSerial}`,workspace_revision:7,job_id:jobId,start_ms:first.start_ms,end_ms:first.end_ms,original_segments:[{...first}],candidate_segments:[{...first,source_text:`候选 ${jobId}`}],context_mode:'isolated',provider_name:'本地测试',model_identity:'test-model',timing_granularities:['segment'],configuration:{}};
   test.previews[value.preview_id]=value;return value;
  };
  window.__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
   const result=await base(cmd,args);
   if(cmd==='list_job_summaries')return [test.job,bJob];
   if(cmd==='get_job_detail')return structuredClone(args.jobId==='job-b'?{...result,job:bJob,segments:bCues}:result);
   if(cmd==='preview_retranscription')return defer('preview',args.request.jobId,preview(args.request.jobId));
   if(cmd==='preview_full_retranscription')return defer('preview',args.jobId,preview(args.jobId));
   if(cmd==='refresh_asr_comparison'){
    const value={...test.previews[args.runId],comparison_id:`comparison-${++test.processingSerial}`,original_segments:segments(args.jobId).slice(0,1).map(c=>({...c}))};
    test.previews[args.runId]=value;return defer('refresh',args.jobId,value);
   }
   if(cmd==='reopen_asr_candidate')return defer('reopen',args.jobId,test.previews[args.runId]);
   if(cmd==='confirm_retranscription'){
    if(test.confirmConflict)throw Error('revision conflict: workspace changed');
    const jobId=args.request.jobId,after=segments(jobId).map((c,i)=>i?{...c}:{...c,source_text:`采用 ${jobId}`,source_edited:true});
    await defer('adopt',jobId,null);segments(jobId).splice(0,segments(jobId).length,...after);return after;
   }
   if(cmd==='asr_candidate_progress')return defer('progress',args.jobId,{phase:'transcribing',completed_ms:1000,duration_ms:2800});
   if(cmd==='list_asr_quality_signals')return defer('signals',args.jobId,result);
   if(cmd==='list_asr_runs')return Object.values(test.previews).filter(p=>p.job_id===args.jobId).map(p=>({id:p.preview_id,provider_name:'测试',model_identity:'test-model',scope_start_ms:0,scope_end_ms:2800,created_at_unix:1,is_candidate:true,status:'succeeded',reopenable:true}));
   return result;
  };
 });
};
