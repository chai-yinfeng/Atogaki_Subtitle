// Regressions that assign native currentTime directly must explicitly load media.
// Demand-loading regressions deliberately do not call this preparation helper.
module.exports = async page => {
 await page.locator('#karaoke-media-host audio, #karaoke-media-host video').evaluate(async media => {
  if (media.readyState >= 1) return;
  await new Promise((resolve,reject) => {
   const clean=()=>{clearTimeout(timer);media.removeEventListener('loadedmetadata',ready);media.removeEventListener('error',failed);};
   const ready=()=>{clean();resolve();},failed=()=>{clean();reject(Error('test media failed to load'));};
   const timer=setTimeout(()=>{clean();reject(Error('test media metadata timeout'));},10000);
   media.addEventListener('loadedmetadata',ready,{once:true});media.addEventListener('error',failed,{once:true});media.load();
  });
 });
};
