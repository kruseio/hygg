use super::*;

/// The filter / search / tag controls form. Without JS it GETs `/app/home`
/// (which server-renders the matching first page); the script enhances it with
/// live updates + infinite scroll.
pub(crate) fn library_controls(
  filter: &str,
  q: &str,
  tag: &str,
  tags: &[String],
) -> String {
  format!(
    r#"<form class="library-controls" method="get" action="/app/home" id="library-controls">
      {filter}
      <input name="q" value="{q}" placeholder="Search title, author, tags" autocomplete="off">
      {tag}
      <button type="submit">Apply</button>
    </form>"#,
    filter = filter_select(filter),
    q = esc(q),
    tag = tag_select(tag, tags),
  )
}

fn filter_select(selected: &str) -> String {
  let mut html = String::from(r#"<select name="filter">"#);
  for (value, label) in [
    ("all", "All documents"),
    ("owned", "My documents"),
    ("org", "Organization"),
  ] {
    html.push_str(&format!(
      r#"<option value="{}"{}>{}</option>"#,
      value,
      if value == selected { " selected" } else { "" },
      label,
    ));
  }
  html.push_str("</select>");
  html
}

fn tag_select(selected: &str, tags: &[String]) -> String {
  let mut html = format!(
    r#"<select name="tag"><option value=""{}>All tags</option>"#,
    if selected.is_empty() { " selected" } else { "" },
  );
  for tag in tags {
    html.push_str(&format!(
      r#"<option value="{}"{}>{}</option>"#,
      esc(tag),
      if tag == selected { " selected" } else { "" },
      esc(tag),
    ));
  }
  html.push_str("</select>");
  html
}

/// Lazy-load + live-filter script. Reads the next offset from the sentinel,
/// then fetches `/app/home/library` pages and appends cards/modals as the
/// sentinel scrolls into view; control changes reset to the first page.
pub(crate) fn library_js() -> &'static str {
  r#"<script>
(function(){
  var form=document.getElementById('library-controls');
  var items=document.getElementById('library-items');
  var modals=document.getElementById('library-modals');
  var sentinel=document.getElementById('library-sentinel');
  if(!form||!items||!sentinel) return;
  var nextOffset=sentinel.dataset.next===''?null:parseInt(sentinel.dataset.next,10);
  var loading=false;
  function params(offset){
    var p=new URLSearchParams();
    p.set('filter',form.filter.value);
    if(form.q.value) p.set('q',form.q.value);
    if(form.tag.value) p.set('tag',form.tag.value);
    p.set('offset',offset);
    return p.toString();
  }
  function load(reset){
    if(loading) return;
    if(!reset && nextOffset===null) return;
    loading=true;
    var offset=reset?0:nextOffset;
    fetch('/app/home/library?'+params(offset),{headers:{'Accept':'application/json'}})
      .then(function(r){return r.json();})
      .then(function(d){
        if(reset){items.innerHTML=d.cards;modals.innerHTML=d.modals;}
        else{items.insertAdjacentHTML('beforeend',d.cards);modals.insertAdjacentHTML('beforeend',d.modals);}
        nextOffset=(d.next===null||d.next===undefined)?null:d.next;
        loading=false;
      }).catch(function(){loading=false;});
  }
  form.addEventListener('submit',function(e){e.preventDefault();load(true);});
  var t;
  form.q.addEventListener('input',function(){clearTimeout(t);t=setTimeout(function(){load(true);},300);});
  form.filter.addEventListener('change',function(){load(true);});
  form.tag.addEventListener('change',function(){load(true);});
  new IntersectionObserver(function(es){if(es[0].isIntersecting)load(false);}).observe(sentinel);
})();
</script>"#
}
