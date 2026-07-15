//! The docs center's two inline client scripts (pure DOM, no deps): the
//! search-as-you-type typeahead on the search box, and the on-page highlighter
//! that jumps to and highlights the matched passage when a page is opened with
//! `?q=`. Kept out of [`docs_view`](super::docs_view) so that file stays within
//! the LOC budget.

/// Search-as-you-type over `/docs/search.json`: debounced fetches render the
/// top matches in a listbox under the input, auto-selecting the first hit so a
/// bare Enter jumps straight to it; Down/Up move the selection, Enter jumps to
/// the highlighted result's section (Enter with the menu closed falls through
/// to the form's full-page `/docs/search`), Escape closes it. Degrades to that
/// plain form submit when scripting is off.
pub(crate) fn typeahead_script() -> &'static str {
  r#"<script>
(function(){
  var input=document.getElementById('doc-search-input');
  if(!input) return;
  var menu=document.getElementById('doc-search-menu');
  var box=input.closest('.doc-search-box');
  var items=[],sel=-1,timer=null,ctrl=null;
  function close(){
    menu.hidden=true; menu.innerHTML=''; items=[]; sel=-1;
    input.setAttribute('aria-expanded','false');
    input.removeAttribute('aria-activedescendant');
  }
  function select(i){
    sel=i;
    [].forEach.call(menu.children,function(li,j){
      var on=j===i; li.classList.toggle('is-active',on);
      if(on){ input.setAttribute('aria-activedescendant',li.id); li.scrollIntoView({block:'nearest'}); }
    });
  }
  function go(i){ if(i>=0&&items[i]) location.assign(items[i].href); }
  function render(hits){
    items=hits;
    if(!hits.length){ close(); return; }
    menu.innerHTML=hits.map(function(h,i){
      return '<li class="doc-search-option" role="option" id="doc-opt-'+i+'">'+
        '<span class="doc-search-crumb"></span>'+
        '<span class="doc-search-snippet">'+h.snippet+'</span></li>';
    }).join('');
    [].forEach.call(menu.children,function(li,i){
      li.querySelector('.doc-search-crumb').textContent=
        hits[i].section?hits[i].page+' › '+hits[i].section:hits[i].page;
      li.addEventListener('mousedown',function(e){ e.preventDefault(); go(i); });
      li.addEventListener('mouseenter',function(){ select(i); });
    });
    menu.hidden=false; input.setAttribute('aria-expanded','true'); select(0);
  }
  function fetchHits(q){
    if(ctrl) ctrl.abort();
    ctrl=new AbortController();
    fetch('/docs/search.json?q='+encodeURIComponent(q),{signal:ctrl.signal})
      .then(function(r){ return r.json(); })
      .then(function(h){ if(input.value.trim()===q) render(h); })
      .catch(function(){});
  }
  input.addEventListener('input',function(){
    var q=input.value.trim();
    if(timer) clearTimeout(timer);
    if(q.length<2){ close(); return; }
    timer=setTimeout(function(){ fetchHits(q); },120);
  });
  input.addEventListener('keydown',function(e){
    if(menu.hidden||!items.length) return;
    if(e.key==='ArrowDown'){ e.preventDefault(); select(sel>=items.length-1?0:sel+1); }
    else if(e.key==='ArrowUp'){ e.preventDefault(); select(sel<=0?items.length-1:sel-1); }
    else if(e.key==='Enter'){ if(sel>=0){ e.preventDefault(); go(sel); } }
    else if(e.key==='Escape'){ e.preventDefault(); close(); }
  });
  document.addEventListener('click',function(e){ if(!box.contains(e.target)) close(); });
})();
</script>"#
}

/// On a page opened with `?q=`, wrap every occurrence of the term in `<mark>`,
/// then jump to and flash-highlight the passage holding the first match *at or
/// after the linked `#section`* (so a hit deep in the page isn't shadowed by an
/// earlier match), scrolling it to center. Pure DOM; degrades to the plain
/// `#heading` anchor when scripting is off.
pub(crate) fn highlight_script() -> &'static str {
  r#"<script>
(function(){
  var q=new URLSearchParams(location.search).get('q');
  if(!q) return;
  var root=document.querySelector('.doc-content');
  if(!root) return;
  var ql=q.toLowerCase();
  var walker=document.createTreeWalker(root,NodeFilter.SHOW_TEXT,null);
  var nodes=[],node;
  while(node=walker.nextNode()){
    if(node.parentNode&&node.parentNode.closest('mark')) continue;
    nodes.push(node);
  }
  var marks=[];
  nodes.forEach(function(n){
    var text=n.nodeValue,lower=text.toLowerCase(),idx=lower.indexOf(ql);
    if(idx<0) return;
    var frag=document.createDocumentFragment(),last=0;
    while(idx>=0){
      if(idx>last) frag.appendChild(document.createTextNode(text.slice(last,idx)));
      var m=document.createElement('mark');
      m.textContent=text.slice(idx,idx+q.length);
      frag.appendChild(m); marks.push(m);
      last=idx+q.length;
      idx=lower.indexOf(ql,last);
    }
    if(last<text.length) frag.appendChild(document.createTextNode(text.slice(last)));
    n.parentNode.replaceChild(frag,n);
  });
  if(!marks.length) return;
  var target=marks[0];
  var id=location.hash?decodeURIComponent(location.hash.slice(1)):'';
  var head=id&&document.getElementById(id);
  if(head){
    for(var i=0;i<marks.length;i++){
      if(head.compareDocumentPosition(marks[i])&Node.DOCUMENT_POSITION_FOLLOWING){ target=marks[i]; break; }
    }
  }
  target.classList.add('is-active');
  var blk=target.closest('p,li,h1,h2,h3,h4,h5,h6,td,th,blockquote,pre')||target;
  blk.classList.add('doc-hit-flash');
  setTimeout(function(){ target.scrollIntoView({block:'center'}); },0);
})();
</script>"#
}
