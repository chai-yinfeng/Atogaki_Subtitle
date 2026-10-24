import test from 'node:test';
import assert from 'node:assert/strict';
import { translationModelChoices } from '../src/translation-provider-view.ts';

test('local translation choices exclude other model kinds and incomplete or unknown downloads',()=>{
 const catalog=[{id:'local',kind:'hy-mt2',name:'Local model'},{id:'asr',kind:'whisper',name:'ASR'}];
 const downloads=[{modelId:'asr',status:'done',path:'/asr'},{modelId:'unknown',status:'done',path:'/unknown'},{modelId:'local',status:'downloading',path:'/partial'},{modelId:'local',status:'done',path:null},{modelId:'local',status:'done',path:'/local'},{modelId:'local',status:'done',path:'/local'}];
 assert.deepEqual(translationModelChoices(catalog,downloads,''),[{path:'/local',name:'Local model'}]);
});

test('external model paths stay opaque and current installed models do not duplicate',()=>{
 assert.deepEqual(translationModelChoices([],[],'C:\\models\\custom<&>.gguf'),[{path:'C:\\models\\custom<&>.gguf',name:'custom<&>.gguf'}]);
 assert.deepEqual(translationModelChoices([{id:'a',kind:'hy-mt2',name:'Installed'}],[{modelId:'a',status:'done',path:'/local'}],'/local'),[{path:'/local',name:'Installed'}]);
});
