import test from 'node:test';
import assert from 'node:assert/strict';
import { createTranslationProviderDrafts, defaultProviderFields } from '../src/translation-provider-drafts.ts';
const profile=(id,model)=>({provider_id:id,model,base_url:null,style:id+' style',readiness:'unverified'});

test('provider defaults never carry another endpoint or model across a switch',()=>{
 const drafts=createTranslationProviderDrafts();drafts.select('openai-compatible');drafts.edit({model:'custom',endpoint:'https://custom.invalid',style:'custom style'});drafts.select('deepl');assert.equal(drafts.current().endpoint,'');assert.equal(drafts.current().model,'');
 drafts.select('gemini');assert.equal(drafts.current().endpoint,defaultProviderFields('gemini').endpoint);drafts.select('openai-compatible');assert.deepEqual(drafts.current(),{model:'custom',endpoint:'https://custom.invalid',style:'custom style'});
});

test('late profiles populate a clean provider even when another provider has dirty fields',()=>{
 const drafts=createTranslationProviderDrafts();drafts.select('deepseek');drafts.edit({model:'A draft',endpoint:'A endpoint',style:'A style'});drafts.select('gemini');drafts.acceptProfiles([profile('deepseek','A saved'),profile('gemini','B saved')]);
 assert.equal(drafts.current().model,'B saved');assert.equal(drafts.current().endpoint,defaultProviderFields('gemini').endpoint);drafts.select('deepseek');assert.equal(drafts.current().model,'A draft');assert.equal(drafts.readiness.get('deepseek'),'unverified');
});

test('only submitted provider and unchanged version are acknowledged',()=>{
 const drafts=createTranslationProviderDrafts();drafts.select('deepseek');drafts.edit(defaultProviderFields('deepseek'));const saved=drafts.snapshot();drafts.edit(defaultProviderFields('deepseek'));drafts.acknowledge(saved,'deepseek');assert.equal(drafts.dirty.has('deepseek'),true);
 const latest=drafts.snapshot();drafts.select('gemini');drafts.edit(defaultProviderFields('gemini'));drafts.acknowledge(latest,'deepseek');assert.equal(drafts.dirty.has('deepseek'),false);assert.equal(drafts.dirty.has('gemini'),true);
});

test('profile refresh preserves drafts but updates clean canonical values after save',()=>{
 const drafts=createTranslationProviderDrafts();drafts.select('deepseek');drafts.edit({model:'draft',endpoint:'custom',style:'style'});const saved=drafts.snapshot();drafts.acceptProfiles([profile('deepseek','old')]);assert.equal(drafts.current().model,'draft');drafts.acknowledge(saved,'deepseek');drafts.acceptProfiles([profile('deepseek','canonical')]);assert.equal(drafts.current().model,'canonical');
});

test('desktop fallback cannot override a profile or a pending provider draft',()=>{
 const drafts=createTranslationProviderDrafts();const fallback={model:'fallback',endpoint:'fallback',style:'fallback'};drafts.fallback('deepseek',fallback);drafts.select('deepseek');assert.deepEqual(drafts.current(),fallback);drafts.acceptProfiles([profile('deepseek','profile')]);drafts.fallback('deepseek',fallback);assert.equal(drafts.current().model,'profile');drafts.edit({model:'draft',endpoint:'draft',style:'draft'});drafts.fallback('deepseek',fallback);assert.equal(drafts.current().model,'draft');
});

test('draft registry copies only non-secret fields and does not expose mutable values',()=>{
 const drafts=createTranslationProviderDrafts();drafts.select('gemini');const fields={model:'model',endpoint:'endpoint',style:'style',apiKey:'test-only'};drafts.edit(fields);fields.model='outside';const value=drafts.current();assert.equal(Object.hasOwn(value,'apiKey'),false);value.model='changed copy';assert.equal(drafts.current().model,'model');
});
