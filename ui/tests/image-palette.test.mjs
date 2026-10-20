import test from 'node:test';
import assert from 'node:assert/strict';
import {extractPalette,paletteVariables,contrast,hex} from '../src/image-palette.ts';
const unhex=value=>[1,3,5].map(i=>parseInt(value.slice(i,i+2),16));
test('palette ignores transparent pixels and a rare saturated outlier',()=>{
  const pixels=[];
  for(let i=0;i<200;i++)pixels.push(60,100,150,255);
  pixels.push(255,0,0,255,0,255,0,0);
  const palette=extractPalette(pixels);
  assert.equal(hex(palette.dominant),'#3c6496');
  assert.equal(hex(palette.accent),'#3c6496');
  assert.throws(()=>extractPalette([0,0,0,0]),/可见颜色/);
});
test('generated reading and button colors remain readable for light dark and gray images',()=>{
  for(const color of [[255,255,255],[0,0,0],[240,220,10],[255,0,100],[0,220,255],[128,128,128]]){
    const vars=paletteVariables({dominant:color,accent:color,swatches:[color]});
    assert.ok(contrast(unhex(vars['--ink']),unhex(vars['--reading-fill']))>=7);
    assert.ok(contrast(unhex(vars['--muted']),unhex(vars['--surface']))>=4.5);
    assert.ok(contrast(unhex(vars['--accent']),[255,250,241])>=4.5);
  }
});
