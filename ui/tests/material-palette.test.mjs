import test from 'node:test';
import assert from 'node:assert/strict';
import {extractMaterialPalette,materialVariables} from '../src/material-palette.ts';
import {contrast,hex} from '../src/image-palette.ts';
const rgb=value=>[1,3,5].map(i=>parseInt(value.slice(i,i+2),16));
test('Material seed extraction ignores transparency and rare saturated noise',()=>{
 const pixels=[];for(let i=0;i<200;i++)pixels.push(60,100,150,255);
 pixels.push(255,0,0,255,0,255,0,0);
 const palette=extractMaterialPalette(pixels);
 assert.equal(hex(palette.accent),'#3c6496');
 assert.throws(()=>extractMaterialPalette([0,0,0,0]),/可见颜色/);
});
test('fixed semantic roles keep light and dark reading, buttons and selections readable',()=>{
 for(const seed of [[255,255,255],[0,0,0],[240,220,10],[255,0,100],[0,220,255],[128,128,128]])for(const dark of [false,true]){
  const vars=materialVariables(seed,dark);
  for(const [text,surface] of [['--foreground','--background'],['--muted-foreground','--card'],['--primary-foreground','--primary'],['--accent-foreground','--accent']]){
   assert.ok(contrast(rgb(vars[text]),rgb(vars[surface]))>=4.5,JSON.stringify({seed,dark,text,surface}));
  }
  assert.equal(vars['--destructive'],dark?'#f87171':'#dc2626');
  assert.notEqual(vars['--primary'],vars['--background']);
 }
});
