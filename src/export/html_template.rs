/// Embedded HTML template for the vis.js knowledge-graph viewer.
/// Placeholders replaced at render time:
/// - `{{TITLE}}`      — page / graph title
/// - `{{GRAPH_DATA}}` — compact JSON blob (`graph_to_value` output)
pub const HTML_TEMPLATE: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1.0">
<title>{{TITLE}}</title>
<script src="https://unpkg.com/vis-network@9.1.9/standalone/umd/vis-network.min.js"></script>
<style>
*{box-sizing:border-box;margin:0;padding:0}
body{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;
     background:#1a1a2e;color:#e0e0e0;height:100vh;
     display:flex;flex-direction:column;overflow:hidden}
#controls{display:flex;align-items:center;gap:8px;padding:8px 12px;
          background:#16213e;border-bottom:1px solid #0f3460;flex-shrink:0}
#controls h1{font-size:14px;color:#e94560;margin-right:4px;
             white-space:nowrap;overflow:hidden;text-overflow:ellipsis;max-width:240px}
#search{flex:1;max-width:260px;padding:5px 10px;border-radius:4px;
        border:1px solid #0f3460;background:#1a1a2e;color:#e0e0e0;font-size:13px}
#search::placeholder{color:#555}
.btn{padding:5px 11px;border-radius:4px;border:1px solid #0f3460;
     background:#0f3460;color:#e0e0e0;cursor:pointer;font-size:12px}
.btn:hover{background:#533483}
#status{font-size:11px;color:#666;margin-left:auto;white-space:nowrap}
#main{display:flex;flex:1;overflow:hidden}
#graph{flex:1}
#sidebar{width:260px;background:#16213e;border-left:1px solid #0f3460;
         padding:12px;overflow-y:auto;display:none;flex-shrink:0}
#sidebar.open{display:block}
#sidebar h3{color:#e94560;margin-bottom:10px;font-size:14px;word-break:break-all}
.meta{font-size:12px;color:#aaa;margin-bottom:4px}
.meta b{color:#ccc}
#nbr-list{margin-top:10px;font-size:12px}
#nbr-list h4{color:#777;margin-bottom:6px}
.nbr{padding:3px 0;border-bottom:1px solid #0f3460;cursor:pointer}
.nbr:hover{color:#e94560}
#legend{background:#16213e;border-left:1px solid #0f3460;
        padding:12px;overflow-y:auto;flex-shrink:0;display:none}
#legend.open{display:block}
#legend h4{font-size:12px;color:#777;margin-bottom:8px}
.li{display:flex;align-items:center;gap:6px;font-size:12px;margin-bottom:4px}
.dot{width:12px;height:12px;border-radius:50%;flex-shrink:0}
</style>
</head>
<body>
<div id="controls">
  <h1>{{TITLE}}</h1>
  <input id="search" type="text" placeholder="Search nodes…"/>
  <button class="btn" onclick="toggleLegend()">Legend</button>
  <button class="btn" onclick="network.fit({animation:true})">Fit</button>
  <span id="status">Loading…</span>
</div>
<div id="main">
  <div id="graph"></div>
  <div id="sidebar"></div>
  <div id="legend"></div>
</div>
<script>
const DATA={{GRAPH_DATA}};
const PALETTE=["#4C72B0","#DD8452","#55A868","#C44E52","#8172B2",
               "#937860","#DA8BC3","#8C8C8C","#CCB974","#64B5CD"];
function clr(c){return c!=null?PALETTE[c%PALETTE.length]:"#8C8C8C"}

// Compute degrees from links
const deg={};
DATA.nodes.forEach(n=>{deg[n.id]=0});
DATA.links.forEach(e=>{
  deg[e.source]=(deg[e.source]||0)+1;
  deg[e.target]=(deg[e.target]||0)+1;
});
const maxDeg=Math.max(1,...Object.values(deg));

const vNodes=new vis.DataSet(DATA.nodes.map(n=>({
  id:n.id,label:n.label,
  color:{background:clr(n.community),border:"#111",
         highlight:{background:"#e94560",border:"#fff"}},
  size:10+30*(deg[n.id]/maxDeg),
  title:"<b>"+n.label+"</b><br/>"+n.file_type+" \u00b7 "+n.source_file+
        (n.community!=null?" \u00b7 Community "+n.community:""),
  font:{color:"#e0e0e0",size:11},
})));

const vEdges=new vis.DataSet(DATA.links.map(e=>{
  const ex=e.confidence==="EXTRACTED";
  return{
    from:e.source,to:e.target,title:e.relation,
    dashes:!ex,width:ex?2:1,
    color:{color:ex?"rgba(200,200,200,0.65)":"rgba(140,140,140,0.30)",
           highlight:"#e94560"},
    font:{size:9,color:"#666",align:"middle"},
    smooth:{type:"continuous"},
  };
}));

const network=new vis.Network(
  document.getElementById("graph"),
  {nodes:vNodes,edges:vEdges},
  {
    physics:{
      solver:"forceAtlas2Based",
      forceAtlas2Based:{
        gravitationalConstant:-60,springConstant:0.08,
        springLength:100,centralGravity:0.01,
      },
      maxVelocity:50,timestep:0.35,
      stabilization:{enabled:true,iterations:200,updateInterval:25},
    },
    interaction:{hover:true,tooltipDelay:200},
    nodes:{shape:"dot",borderWidth:1.5},
    edges:{arrows:{to:{enabled:true,scaleFactor:0.5}}},
  }
);

network.on("stabilizationProgress",function(p){
  document.getElementById("status").textContent=
    "Stabilising "+Math.round(p.iterations/p.total*100)+"%";
});
network.on("stabilizationIterationsDone",function(){
  document.getElementById("status").textContent=
    DATA.nodes.length+" nodes \u00b7 "+DATA.links.length+" edges";
  network.setOptions({physics:{enabled:false}});
});
network.on("click",function(p){
  if(p.nodes.length)showNode(p.nodes[0]);else closeSidebar();
});

function showNode(id){
  var n=DATA.nodes.find(function(x){return x.id===id});
  if(!n)return;
  var conns=DATA.links
    .filter(function(e){return e.source===id||e.target===id})
    .map(function(e){
      var oid=e.source===id?e.target:e.source;
      var o=DATA.nodes.find(function(x){return x.id===oid});
      return"<div class='nbr' onclick='focusNode(\""+oid+"\")'>"+(o?o.label:oid)+
             " <span style='color:#555'>("+e.relation+")</span></div>";
    }).join("");
  document.getElementById("sidebar").innerHTML=
    "<h3>"+n.label+"</h3>"+
    "<div class='meta'><b>Type:</b> "+n.file_type+"</div>"+
    "<div class='meta'><b>File:</b> "+n.source_file+"</div>"+
    "<div class='meta'><b>Community:</b> "+(n.community!=null?n.community:"\u2014")+"</div>"+
    "<div class='meta'><b>Degree:</b> "+deg[id]+"</div>"+
    "<div id='nbr-list'><h4>Connections ("+deg[id]+")</h4>"+conns+"</div>";
  document.getElementById("sidebar").classList.add("open");
}
function closeSidebar(){document.getElementById("sidebar").classList.remove("open")}
function focusNode(id){
  network.focus(id,{scale:1.5,animation:true});
  network.selectNodes([id]);
  showNode(id);
}

document.getElementById("search").addEventListener("input",function(e){
  var q=e.target.value.toLowerCase();
  if(!q){network.unselectAll();return}
  var m=DATA.nodes.filter(function(n){return n.label.toLowerCase().indexOf(q)>=0})
           .map(function(n){return n.id});
  network.selectNodes(m);
  if(m.length===1)focusNode(m[0]);
});

var legendOpen=false;
function toggleLegend(){
  legendOpen=!legendOpen;
  var el=document.getElementById("legend");
  if(legendOpen){
    var cs=[...new Set(DATA.nodes.map(function(n){return n.community})
            .filter(function(c){return c!=null}))].sort(function(a,b){return a-b});
    el.innerHTML="<h4>Communities ("+cs.length+")</h4>"+
      cs.map(function(c){
        return"<div class='li'><div class='dot' style='background:"+clr(c)+"'></div>Community "+c+"</div>";
      }).join("");
    el.classList.add("open");
  }else{el.classList.remove("open")}
}
</script>
</body>
</html>
"##;
