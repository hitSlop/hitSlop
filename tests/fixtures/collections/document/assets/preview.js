// Minimal synchronous capture fixture for shared host/engine integration tests.
globalThis.__slopCapture = json => {
  const request = JSON.parse(json);
  const escape = value => String(value ?? '').replace(/[&<>"']/g, character => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[character]));
  const title = escape(request.snapshot.title ?? 'Document fixture');
  const rows = (request.snapshot.rows ?? []).map(row => '<p>' + escape(row.text) + '</p>').join('');
  return JSON.stringify({fonts:[],frames:[{
    kind:request.kind,width:request.width,height:request.height,
    html:'<html data-slop-renderer="true" data-slop-capture="static"><body><h1>'+title+'</h1>'+rows+'</body></html>',
    css:'html,body{margin:0;width:100%;background:white;color:#111;font-family:sans-serif}body{padding:24px;box-sizing:border-box}h1{font-size:24px}p{font-size:16px}'
  }]});
};
