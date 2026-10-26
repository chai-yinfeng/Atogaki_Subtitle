import test from 'node:test';
import assert from 'node:assert/strict';
import { createStructureEditBackend } from '../src/editor-structure-backend.ts';
const cue = { id:'cue',job_id:'job',segment_index:0,start_ms:0,end_ms:1000,source_text:'source',translated_text:null,source_edited:false,translation_edited:false,translation_stale:false,timing_edited:false };
const descriptor = {token:'opaque',label:'Cut',expected_revision:42};
const result = {segments:[cue],workspace_revision:42,translation_context_stale_ids:['consumer'],undo:{latest:descriptor,discarded_entries:2,oversized:false},warnings:['已采用，但标记写入失败']};

test('all recorded edits preserve revision, dependency status, budget outcome and post-commit warnings',async()=>{
 const calls=[];
 const backend=createStructureEditBackend(async(command,args)=>{calls.push({command,args});return result;});
 assert.equal(await backend.saveTiming('job',[cue],[{...cue,end_ms:900}]),result);
 assert.equal(await backend.editBlock('job',[cue],'cue',null),result);
 const cut={jobId:'job',expectedRevision:41,segmentId:'cue',boundaryMs:500,leftSourceText:'left',rightSourceText:'right',leftTranslatedText:null,rightTranslatedText:null};
 assert.equal(await backend.split(cut),result);
 const join={jobId:'job',expectedRevision:42,leftSegmentId:'cue',rightSegmentId:'next',sourceText:'joined',translatedText:null};
 assert.equal(await backend.merge(join),result);
 assert.equal(await backend.adopt('job','run','comparison'),result);
 assert.deepEqual(calls,[
  {command:'save_subtitle_timing_recorded',args:{request:{jobId:'job',beforeSegments:[cue],afterSegments:[{...cue,end_ms:900}]}}},
  {command:'edit_subtitle_block_recorded',args:{request:{jobId:'job',beforeSegments:[cue],removeId:'cue',insertion:null}}},
  {command:'split_subtitle_recorded',args:{request:cut}},
  {command:'merge_subtitles_recorded',args:{request:join}},
  {command:'confirm_retranscription_recorded',args:{request:{jobId:'job',previewId:'run',comparisonId:'comparison'}}},
 ]);
});
test('state is on-demand; undo sends only task and token and never retries or falls back after failure',async()=>{
 const calls=[],state={segments:[cue],workspace_revision:42,latest:descriptor};let failed=true;
 const backend=createStructureEditBackend(async(command,args)=>{
  calls.push({command,args});
  if(command==='get_structure_edit_state')return state;
  if(failed)throw Error('workspace changed');
  return {segments:[],workspace_revision:43,translation_context_stale_ids:[],latest:null,discarded_entries:0};
 });
 assert.deepEqual(calls,[]);
 assert.equal(await backend.state('job'),state);
 await assert.rejects(backend.undo('job',descriptor.token),/workspace changed/);
 assert.equal(calls.length,2);
 failed=false;
 const restored=await backend.undo('job',descriptor.token);
 assert.equal(restored.workspace_revision,43);
 assert.deepEqual(calls,[
  {command:'get_structure_edit_state',args:{jobId:'job'}},
  {command:'undo_subtitle_structure',args:{request:{jobId:'job',token:'opaque'}}},
  {command:'undo_subtitle_structure',args:{request:{jobId:'job',token:'opaque'}}},
 ]);
});
