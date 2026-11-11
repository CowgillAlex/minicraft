#!/usr/bin/env python3
"""Build an offline screenshot gallery, a PNG ZIP, and a compact inline preview."""
import argparse
import base64
import json
from pathlib import Path
import zipfile

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--input',type=Path,required=True)
parser.add_argument('--output',type=Path,required=True)
parser.add_argument('--resources',type=Path,help='Uploaded PNG metadata for a gallery that loads photos on demand')
args=parser.parse_args()
args.output.mkdir(parents=True,exist_ok=True)
remote={}
if args.resources:
    from urllib.parse import urlparse
    for item in json.loads(args.resources.read_text())['items']:
        url=item['url']
        if urlparse(url).scheme!='https' or not urlparse(url).hostname:raise ValueError('Screenshot URLs must use HTTPS and include a hostname')
        remote[item['name'].removeprefix('minicraft-v0.15.0-')]=url
shots=json.loads((args.input/'manifest.json').read_text())
shots += [dict(name='natural-forest',file='natural-forest.png',kind='generated world',seed='1794358123416304200',origin='-1812,48,-2068',height=40,view='world-surface'),
          dict(name='natural-ocean',file='natural-ocean.png',kind='generated world',seed='1794358123416304200',origin='-1556,32,-2068',height=40,view='world-underwater',night_vision=True)]
curated=['natural-forest','warm_ocean','branching-oak','flower-parts','wheat-grains','grass-seeds','cottage-night','diamond-inclusions']
notes={'branching-oak':'Tapered trunk, roots, branching limbs, fine shoots and individual leaf sprays.',
       'flower-parts':'Separate stem, leaves, petals and pollen voxels.', 'wheat-grains':'Jointed stalks with individual grain kernels in their heads.',
       'grass-seeds':'Grass blades and actual seed kernels.', 'cottage-night':'A hollow timber cottage with warm interior lighting, glass windows and an editable door.',
       'natural-forest':'Generated forest terrain, with production feature placement. This is a finite crop of the infinite source field.',
       'natural-ocean':'Generated ocean floor and kelp. Night vision is enabled to reveal underwater detail.',
       'mineral-inclusions':'Cut-open host rock shows discrete mineral inclusions, leaving ordinary stone between the clusters.'}
for shot in shots:
    name=shot['name']
    shot['title']=name.replace('-',' ').replace('_',' ').capitalize()
    shot['caption']=notes.get(name,'Mineral inclusions in a cut-open rock specimen.' if name.endswith('-inclusions') else 'Production feature recipes arranged on a controlled reference plot. Natural placement follows terrain and biome rules.')
    shot['group']='World' if shot['kind']=='generated world' else 'Minerals' if 'inclusions' in name else 'Cottages' if 'cottage' in name else 'Plants' if name in ['plant-parts','flower-parts','wheat-grains','grass-seeds','fern-fronds'] else 'Trees' if name=='branching-oak' else 'Underwater' if 'ocean' in name or 'river' in name else 'Caves' if 'caves' in name or name=='deep_dark' else 'Biomes'
rank={name:i for i,name in enumerate(curated)}
shots.sort(key=lambda s:(rank.get(s['name'],99),s['name']))
CSS='''*{box-sizing:border-box}body{font-family:var(--font-sans,system-ui);color:var(--foreground);margin:0}main{width:100%}h1{font-size:18px;margin:0 0 6px}p{font-size:13px;line-height:1.5;color:var(--muted-foreground);margin:5px 0 10px}.filters{display:flex;flex-wrap:wrap;gap:6px;margin:12px 0}button,input,select{font:inherit;font-size:12px;border:1px solid var(--border);border-radius:var(--radius,6px);background:var(--secondary);color:var(--foreground);padding:7px 10px}button{cursor:pointer}button.active{border-color:var(--primary);color:var(--primary)}.toolbar{display:flex;gap:7px;margin-bottom:10px;align-items:center}input{width:130px;min-width:0}select{flex:1;min-width:0}#counter{font:12px var(--font-mono,monospace);white-space:nowrap;color:var(--muted-foreground)}img{display:block;width:100%;aspect-ratio:1100/760;object-fit:contain;background:var(--code-background);border-radius:var(--radius,6px)}h2{font-size:15px;margin:12px 0 4px}.meta{font-size:11px;color:var(--muted-foreground)}a{color:var(--primary)}.links{display:flex;gap:16px;font-size:12px;margin-top:10px} @media(max-width:500px){input{width:100px}.toolbar{flex-wrap:wrap}select{flex-basis:65%}button{padding:6px 8px}}'''
JS='''const all=DATA;let group='All',cursor=0,filtered=all;const $=id=>document.getElementById(id);function filter(){const q=$('search').value.toLowerCase();filtered=all.filter(s=>(group==='All'||s.group===group)&&s.title.toLowerCase().includes(q));cursor=0;$('select').replaceChildren(...filtered.map((s,i)=>{const o=document.createElement('option');o.value=i;o.textContent=s.title;return o}));show()}function show(){if(!filtered.length){$('image').hidden=true;$('title').textContent='No images match';$('caption').textContent='';$('counter').textContent='0 / 0';return}$('image').hidden=false;cursor=(cursor+filtered.length)%filtered.length;const s=filtered[cursor];$('image').src=s.src;$('image').alt=s.title;$('title').textContent=s.title;$('caption').textContent=s.caption;$('meta').textContent=s.kind+(s.seed?' · seed '+s.seed+' · origin '+s.origin:'')+(s.night_vision?' · night vision':'');$('counter').textContent=(cursor+1)+' / '+filtered.length;$('select').value=cursor;$('download').href=s.src;$('download').download=s.file}for(const name of ['All',...new Set(all.map(s=>s.group))]){const b=document.createElement('button');b.textContent=name;b.className=name===group?'active':'';b.onclick=()=>{group=name;for(const c of $('filters').children)c.classList.toggle('active',c===b);filter()};$('filters').append(b)}$('prev').onclick=()=>{cursor--;show()};$('next').onclick=()=>{cursor++;show()};$('select').onchange=()=>{cursor=+$('select').value;show()};$('search').oninput=filter;document.addEventListener('keydown',e=>{if(e.target.tagName==='INPUT')return;if(e.key==='ArrowRight'){$('next').click();e.preventDefault()}if(e.key==='ArrowLeft'){$('prev').click();e.preventDefault()}});filter();'''
def html(rows,embedded,inline=False,on_demand=False):
    rows=[dict(s,src=('data:image/png;base64,'+base64.b64encode((args.input/s['file']).read_bytes()).decode()) if embedded else 'images/'+s['file'])for s in rows]
    if on_demand:
        for i,row in enumerate(rows):
            if i: row['src']=remote[row['file']]
    theme='' if inline else ':root{color-scheme:dark;--background:#0e1518;--foreground:#e8efec;--muted-foreground:#9aaca6;--secondary:#192327;--border:#33433e;--primary:#7bd6b1;--code-background:#0b1113;--radius:7px}html{background:var(--background)}body{max-width:1100px;margin:auto;padding:24px}'
    intro='A selection from the full 75-image gallery.' if inline else '75 captures: every biome treatment, detailed plant parts, cottages, minerals and two generated-world scenes. Use the arrow keys to browse.'
    return '<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>MiniCraft v0.15.0 screenshot gallery</title><style>'+theme+CSS+'</style></head><body><main><h1>MiniCraft · life at voxel scale</h1><p>'+intro+'</p><div id="filters" class="filters"></div><div class="toolbar"><input id="search" aria-label="Find a capture" placeholder="Find a capture"><select id="select" aria-label="Choose a capture"></select><button id="prev" aria-label="Previous capture">←</button><span id="counter"></span><button id="next" aria-label="Next capture">→</button></div><img id="image"><h2 id="title"></h2><p id="caption"></p><div id="meta" class="meta"></div><div class="links"><a id="download">Download this PNG</a>EXTRA_LINKS</div></main><script>const DATA='+json.dumps(rows,separators=(',',':'))+';'+JS+'</script></body></html>'
full=args.output/'minicraft-v0.15.0-gallery.html'
full.write_text(html(shots,True,on_demand=bool(remote)).replace('EXTRA_LINKS',''))
inline=args.output/'inline-gallery.html'
inline.write_text(html([s for s in shots if s['name'] in curated],True,True))
(args.output/'manifest.json').write_text(json.dumps(shots,indent=2)+'\n')
with zipfile.ZipFile(args.output/'minicraft-v0.15.0-screenshots.zip','w',zipfile.ZIP_DEFLATED)as z:
    z.writestr('index.html',html(shots,False).replace('EXTRA_LINKS',''))
    z.write(args.output/'manifest.json','manifest.json')
    for shot in shots:z.write(args.input/shot['file'],'images/'+shot['file'])
print(f'{len(shots)} captures; gallery {full.stat().st_size/1048576:.1f} MiB')
