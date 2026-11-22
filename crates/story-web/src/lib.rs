//! Optional loopback-only browser view for planning and read-only manuscript review.
use serde::Deserialize;
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener},
    str::FromStr,
};
use story_app::WriterApp;

#[derive(Deserialize)]
struct CreateSceneRequest {
    chapter_id: String,
    title: String,
    position: u32,
}
#[derive(Deserialize)]
struct CreateChapterRequest {
    title: String,
    parent_id: Option<String>,
    position: u32,
}
#[derive(Deserialize)]
struct MoveSceneRequest {
    chapter_id: String,
    position: u32,
}
#[derive(Deserialize)]
struct PlanningUpdateRequest {
    title: Option<String>,
    synopsis: Option<String>,
    status: Option<String>,
}
#[derive(Deserialize)]
struct EventTimeUpdateRequest {
    start: String,
    end: String,
    source_time: String,
}
#[derive(Deserialize)]
struct SceneTimeUpdateRequest {
    start: String,
    end: String,
}
#[derive(Deserialize)]
struct CreateIssueRequest {
    title: String,
    scene_id: Option<String>,
}
#[derive(Deserialize)]
struct IssueStatusRequest {
    status: String,
}

const PAGE: &str = r#"<!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width">
<title>Writer's IDE</title><style>
body{font:16px system-ui;margin:0;color:#20242a;background:#f3f5f7}header{padding:1rem 2rem;background:#24354a;color:white}main{display:grid;grid-template-columns:minmax(18rem,32%) 1fr;gap:1rem;padding:1rem;max-width:1400px;margin:auto}section{background:white;border:1px solid #ccd2d9;border-radius:.4rem;padding:1rem}button{margin:.3rem 0;padding:.55rem;cursor:pointer}button.scene{display:block;width:100%;text-align:left;border:0;border-bottom:1px solid #e5e8eb;background:white;padding:.7rem}button.scene:hover{background:#eef3f8}textarea, input, select{box-sizing:border-box;width:100%;padding:.5rem;margin:.2rem 0  .5rem}pre{white-space:pre-wrap;overflow-wrap:anywhere;font:inherit;line-height:1.6}small{color:#52606d}.notice{min-height:1.4rem;color:#234f73}.graph{overflow:auto;border:1px solid #d7dce2;border-radius:.3rem}.graph svg{display:block;width:100%;min-width:760px}.graph line{stroke:#9aa7b4;stroke-width:1.5}.graph rect{rx:6;stroke:#617181;stroke-width:1}.graph text{font:12px system-ui;fill:#17202a}.graph g[data-scene-id]{cursor:pointer}.graph g[data-scene-id]:hover rect{stroke:#1769aa;stroke-width:3}.graph .group rect{fill:#f0f2f5}.graph .scene rect{fill:#e7f1fb}.graph .character rect{fill:#e8f5ec}.graph .plotline rect{fill:#fff2d9}.graph .world rect{fill:#f4eafb}.graph .knowledge rect{fill:#fff0dc}@media(max-width:750px){main{grid-template-columns:1fr}}</style>
<header><h1 id="title">Writer's IDE</h1><p>Local planning graph · manuscript prose remains read-only here · <a href="/preview" target="_blank" rel="noopener">Open compiled manuscript preview</a> · <a href="/downloads">Download compiled EPUB/DOCX/PDF</a></p></header><main>
<section><h2>Scenes</h2><label>Search<input id="scene-filter-text" type="search" placeholder="Title or synopsis"></label><label>Status<select id="scene-filter-status"><option value="">Any status</option><option>idea</option><option>outlined</option><option>ready_to_draft</option><option>drafting</option><option>draft_complete</option><option>revising</option><option>complete</option><option>archived</option></select></label><label>Character<select id="scene-filter-character"><option value="">Any character</option></select></label><label>Plotline<select id="scene-filter-plot"><option value="">Any plotline</option></select></label><div id="scenes">Loading…</div><h3>Add chapter</h3><form id="create-chapter"><input id="new-chapter-title" required placeholder="Chapter title"><button>Add chapter</button></form><h3>Add scene</h3><form id="create-scene"><select id="new-chapter"></select><input id="new-title" required placeholder="Scene title"><button>Add scene</button></form></section>
<section><h2 id="scene-title">Select a scene</h2><small id="scene-meta"></small><p><label>Scene title<input id="scene-name"></label><label>Planning synopsis<textarea id="synopsis" rows="4"></textarea></label><label>Status<select id="status"><option>idea</option><option>outlined</option><option>ready_to_draft</option><option>drafting</option><option>draft_complete</option><option>revising</option><option>complete</option></select></label><button id="save-planning">Save planning</button></p><p><label>Story start time (ticks)<input id="scene-start-time" placeholder="? or integer / interval"></label><label>Story end time (ticks)<input id="scene-end-time" placeholder="? or integer / interval"></label><button id="save-scene-time">Save chronology</button></p><p><label>Move to chapter<select id="move-chapter"></select></label><label>Position<input id="move-position" type="number" min="0" value="0"></label><button id="move-scene">Move scene</button></p><p class="notice" id="notice"></p><h3>Read-only manuscript</h3><pre id="prose">Select a scene to inspect its text.</pre></section>
<section><h2>Characters</h2><div id="characters"></div></section><section><h2>Plotlines</h2><div id="plotlines"></div></section><section><h2>Locations and events</h2><div id="world"></div></section><section><h2>Research and issues</h2><form id="create-issue"><input id="new-issue-title" required placeholder="Issue title"><button>Create issue</button></form><div id="work"></div></section><section><h2>Story structure and graph</h2><div id="structure"></div><div class="graph"><svg id="graph" role="img" aria-label="Story relationship and approved knowledge graph"></svg></div><div id="edges"></div><div id="knowledge"></div></section></main>
<script>
let plan,selected;async function api(path,method='GET',data){const r=await fetch(path,{method,headers:data?{'Content-Type':'application/json'}:{},body:data?JSON.stringify(data):undefined});const j=await r.json().catch(()=>({}));if(!r.ok)throw Error(j.error||r.statusText);return j}
function renderScenes(){const query=document.querySelector('#scene-filter-text').value.trim().toLocaleLowerCase();const status=document.querySelector('#scene-filter-status').value;const character=document.querySelector('#scene-filter-character').value;const plot=document.querySelector('#scene-filter-plot').value;const matches=plan.scenes.filter(s=>(!query||`${s.title} ${s.synopsis||''}`.toLocaleLowerCase().includes(query))&&(!status||s.status===status)&&(!character||s.characters.some(x=>x.id===character))&&(!plot||s.plotlines.some(x=>x.id===plot)));document.querySelector('#scenes').innerHTML=matches.map(s=>`<button class="scene" data-id="${s.id}"><b>${esc(s.title)}</b><br><small>${esc(s.chapter||'Unplaced')} · ${s.actual_words} words · ${esc(s.status)}</small><p>${esc(s.synopsis||'')}</p></button>`).join('')||'No matching scenes';document.querySelectorAll('#scenes [data-id]').forEach(b=>b.onclick=()=>openScene(b.dataset.id))}
async function load(){plan=await api('/api/plan');document.querySelector('#title').textContent=plan.project.title;document.querySelector('#scene-filter-character').innerHTML='<option value="">Any character</option>'+plan.characters.map(x=>`<option value="${esc(x.id)}">${esc(x.name)}</option>`).join('');document.querySelector('#scene-filter-plot').innerHTML='<option value="">Any plotline</option>'+plan.plotlines.map(x=>`<option value="${esc(x.id)}">${esc(x.name)}</option>`).join('');renderScenes();document.querySelector('#characters').innerHTML=plan.characters.map(x=>`<p>${esc(x.name)} <small>(${x.scene_count} scenes)</small></p>`).join('')||'No characters';document.querySelector('#plotlines').innerHTML=plan.plotlines.map(x=>`<p>${esc(x.name)} <small>(${x.scene_count} scenes)</small></p>`).join('')||'No plotlines';document.querySelector('#world').innerHTML=[...plan.locations.map(x=>`<p>Location · ${esc(x.name)}</p>`),...plan.events.map(x=>`<p><button data-event="${esc(x.id)}" data-start="${esc(timeLabel(x.start))}" data-end="${esc(timeLabel(x.end))}" data-source="${esc(x.source_time)}">Edit time · ${esc(x.title)}</button> <small>${esc(x.source_time)} · ${esc(timeLabel(x.start))} → ${esc(timeLabel(x.end))}</small></p>`)].join('')||'No locations or events';document.querySelector('#work').innerHTML=[...plan.research.map(x=>`<p>Research · ${esc(x.title)}</p>`),...plan.issues.map(x=>`<p>Issue · ${esc(x.title)} <small>${esc(x.status)}</small><select data-issue-status="${esc(x.id)}"><option value="open" ${x.status==='open'?'selected':''}>Open</option><option value="in_progress" ${x.status==='in_progress'?'selected':''}>In progress</option><option value="resolved" ${x.status==='resolved'?'selected':''}>Resolved</option><option value="deferred" ${x.status==='deferred'?'selected':''}>Deferred</option><option value="closed" ${x.status==='closed'?'selected':''}>Closed</option></select></p>`)].join('')||'No research or issues';document.querySelector('#structure').innerHTML=plan.groups.map(x=>`<p>${'　'.repeat(x.depth)}${esc(x.title)} <small>${esc(x.kind)}</small></p>`).join('')||'No structure';drawGraph();document.querySelector('#edges').innerHTML=`<small>${plan.edges.length} stable-ID relationships in the project graph</small>`;document.querySelector('#knowledge').innerHTML=plan.claims.map(x=>`<p><b>${esc(x.name)}</b> · ${esc(x.claim)}<br><small>Evidence: ${esc(x.evidence_quote)} · ${esc(x.source_kind)} ${esc(x.source_id)}:${x.evidence_start_byte}..${x.evidence_end_byte}</small></p>`).join('')||'No approved knowledge claims';const chapters=plan.groups.filter(x=>x.kind==='chapter');for(const id of ['new-chapter','move-chapter'])document.querySelector('#'+id).innerHTML=chapters.map(c=>`<option value="${c.id}">${esc(c.title)}</option>`).join('');document.querySelectorAll('[data-id]').forEach(b=>b.onclick=()=>openScene(b.dataset.id))}
function drawGraph(){const columns=[['group',plan.groups.map(n=>({...n,label:n.title}))],['scene',plan.scenes.map(n=>({...n,label:n.title}))],['character',plan.characters.map(n=>({...n,label:n.name}))],['plotline',plan.plotlines.map(n=>({...n,label:n.name}))],['world',[...plan.locations.map(n=>({...n,label:'Location · '+n.name})),...plan.events.map(n=>({...n,label:'Event · '+n.title}))]],['knowledge',plan.claims.map(n=>({...n,label:'Fact · '+n.name}))]];const nodes=[],positions=new Map();let height=180;columns.forEach(([kind,items],col)=>{items.forEach((n,row)=>{const x=20+col*190,y=24+row*54;const node={id:String(n.id),kind,label:n.label,x,y};nodes.push(node);if(!positions.has(node.id))positions.set(node.id,node);height=Math.max(height,y+52)})});const lines=plan.edges.map(e=>{const a=positions.get(String(e.from)),b=positions.get(String(e.to));return a&&b?`<line x1="${a.x+160}" y1="${a.y+16}" x2="${b.x}" y2="${b.y+16}"><title>${esc(e.kind)}</title></line>`:''}).join('');const cards=nodes.map(n=>`<g class="${n.kind}" ${n.kind==='scene'?`data-scene-id="${esc(n.id)}"`:''}><rect x="${n.x}" y="${n.y}" width="160" height="32"></rect><text x="${n.x+8}" y="${n.y+21}">${esc(n.label.slice(0,22))}</text><title>${esc(n.label)}</title></g>`).join('');const svg=document.querySelector('#graph');svg.setAttribute('viewBox',`0 0 ${20+columns.length*190} ${height}`);svg.setAttribute('height',height);svg.innerHTML=lines+cards;svg.querySelectorAll('[data-scene-id]').forEach(node=>node.onclick=()=>openScene(node.dataset.sceneId))}
async function openScene(id){selected=id;const s=await api('/api/scenes/'+encodeURIComponent(id));document.querySelector('#scene-title').textContent=s.title;document.querySelector('#scene-meta').textContent=`${s.actual_words} words · ${s.status} · POV ${s.pov||'unset'}`;document.querySelector('#scene-name').value=s.title;document.querySelector('#synopsis').value=s.synopsis;document.querySelector('#status').value=s.status;document.querySelector('#scene-start-time').value=timeLabel(s.start_time);document.querySelector('#scene-end-time').value=timeLabel(s.end_time);document.querySelector('#prose').textContent=s.text}
document.querySelector('#create-issue').onsubmit=async e=>{e.preventDefault();try{await api('/api/issues','POST',{title:document.querySelector('#new-issue-title').value,scene_id:selected||null});document.querySelector('#new-issue-title').value='';await load();notice('Issue created')}catch(e){notice(e.message)}};document.querySelector('#work').addEventListener('change',async e=>{const select=e.target.closest('[data-issue-status]');if(!select)return;try{await api('/api/issues/'+encodeURIComponent(select.dataset.issueStatus)+'/status','POST',{status:select.value});await load();notice('Issue status saved')}catch(error){notice(error.message)}});
document.querySelector('#create-scene').onsubmit=async e=>{e.preventDefault();try{const chapter=document.querySelector('#new-chapter').value;const position=plan.scenes.filter(s=>s.chapter_id===chapter).length;const s=await api('/api/scenes','POST',{chapter_id:chapter,title:document.querySelector('#new-title').value,position});document.querySelector('#new-title').value='';await load();await openScene(s.id);notice('Scene created')}catch(e){notice(e.message)}};
document.querySelector('#create-chapter').onsubmit=async e=>{e.preventDefault();try{await api('/api/chapters','POST',{title:document.querySelector('#new-chapter-title').value,position:plan.groups.filter(g=>g.kind==='chapter').length});document.querySelector('#new-chapter-title').value='';await load();notice('Chapter created')}catch(e){notice(e.message)}};
document.querySelector('#save-planning').onclick=async()=>{if(!selected)return;try{await api('/api/scenes/'+selected+'/planning','POST',{title:document.querySelector('#scene-name').value,synopsis:document.querySelector('#synopsis').value,status:document.querySelector('#status').value});await load();await openScene(selected);notice('Planning metadata saved')}catch(e){notice(e.message)}};
document.querySelector('#save-scene-time').onclick=async()=>{if(!selected)return;try{await api('/api/scenes/'+selected+'/time','POST',{start:document.querySelector('#scene-start-time').value,end:document.querySelector('#scene-end-time').value});await load();await openScene(selected);notice('Scene chronology saved')}catch(e){notice(e.message)}};
document.querySelector('#move-scene').onclick=async()=>{if(!selected)return;try{await api('/api/scenes/'+selected+'/move','POST',{chapter_id:document.querySelector('#move-chapter').value,position:Number(document.querySelector('#move-position').value)});await load();await openScene(selected);notice('Scene moved; prose and story chronology are unchanged')}catch(e){notice(e.message)}};
function notice(s){document.querySelector('#notice').textContent=s}function esc(s){return String(s).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]))}document.querySelector('#world').addEventListener('click',e=>{const b=e.target.closest('[data-event]');if(b)editEventTime(b)});
function timeLabel(t){if(t.precision==='Unknown')return '?';if(t.precision==='Bounded')return `${t.lower}..${t.upper}`;if(t.precision==='Approximate')return `~${t.nominal},${t.lower},${t.upper}`;return String(t.nominal)}
async function editEventTime(b){const start=prompt('Start: tick, ?, ~nominal,lower,upper, or lower..upper',b.dataset.start);if(start===null)return;const end=prompt('End: tick, ?, ~nominal,lower,upper, or lower..upper',b.dataset.end);if(end===null)return;const source=prompt('Source time label',b.dataset.source);if(source===null)return;try{await api(`/api/events/${encodeURIComponent(b.dataset.event)}/time`,'POST',{start,end,source_time:source});await load();notice('Event chronology saved')}catch(e){notice(e.message)}}
for(const id of ['scene-filter-text','scene-filter-status','scene-filter-character','scene-filter-plot'])document.querySelector('#'+id).addEventListener(id==='scene-filter-text'?'input':'change',renderScenes);load().catch(e=>notice(e.message));
</script></html>"#;

#[derive(Debug)]
pub struct BrowserServer {
    address: SocketAddr,
    listener: TcpListener,
}

impl BrowserServer {
    /// Always binds to loopback; callers may select a port for SSH forwarding.
    pub fn bind(port: u16) -> std::io::Result<Self> {
        let listener = TcpListener::bind(Self::loopback_address(port))?;
        Ok(Self {
            address: listener.local_addr()?,
            listener,
        })
    }
    pub fn loopback_address(port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)
    }
    pub fn address(&self) -> SocketAddr {
        self.address
    }
    pub fn serve(self, app: &mut WriterApp) -> std::io::Result<()> {
        for stream in self.listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
                    let mut reader = BufReader::new(stream.try_clone()?);
                    let mut line = String::new();
                    reader.read_line(&mut line)?;
                    let mut parts = line.split_whitespace();
                    let method = parts.next().unwrap_or_default();
                    let target = parts.next().unwrap_or("/");
                    let mut content_length = 0usize;
                    let mut host = None;
                    let mut origin = None;
                    loop {
                        let mut header = String::new();
                        if reader.read_line(&mut header)? == 0 || header == "\r\n" || header == "\n"
                        {
                            break;
                        }
                        if let Some((name, value)) = header.split_once(':')
                            && name.eq_ignore_ascii_case("content-length")
                        {
                            content_length = value.trim().parse().unwrap_or(0);
                        }
                        if let Some((name, value)) = header.split_once(':')
                            && name.eq_ignore_ascii_case("host")
                        {
                            host = Some(value.trim().to_owned());
                        }
                        if let Some((name, value)) = header.split_once(':')
                            && name.eq_ignore_ascii_case("origin")
                        {
                            origin = Some(value.trim().to_owned());
                        }
                    }
                    if !host
                        .as_deref()
                        .is_some_and(|host| valid_host(host, self.address))
                        || origin.as_deref().is_some_and(|origin| {
                            !host
                                .as_deref()
                                .is_some_and(|host| valid_origin(origin, host))
                        })
                    {
                        write_response(
                            &mut stream,
                            403,
                            "text/plain; charset=utf-8",
                            "Loopback host/origin check failed",
                        )?;
                        continue;
                    }
                    if content_length > 16_384 {
                        write_response(
                            &mut stream,
                            413,
                            "text/plain; charset=utf-8",
                            "Request too large",
                        )?;
                        continue;
                    }
                    let mut body = vec![0; content_length];
                    reader.read_exact(&mut body)?;
                    let body = std::str::from_utf8(&body).unwrap_or_default();
                    if let Some(index) = target
                        .split('?')
                        .next()
                        .unwrap_or(target)
                        .strip_prefix("/downloads/")
                    {
                        if method != "GET" {
                            write_response(
                                &mut stream,
                                405,
                                "text/plain; charset=utf-8",
                                "Method not allowed",
                            )?;
                            continue;
                        }
                        let (status, mime, filename, bytes) =
                            match download_compile_artifact(app, index) {
                                Ok(Some(artifact)) => {
                                    (200, artifact.mime, Some(artifact.filename), artifact.bytes)
                                }
                                Ok(None) => (
                                    404,
                                    "text/plain; charset=utf-8",
                                    None,
                                    b"Compile download not found".to_vec(),
                                ),
                                Err(error) => {
                                    (500, "text/plain; charset=utf-8", None, error.into_bytes())
                                }
                            };
                        write_binary_response(
                            &mut stream,
                            status,
                            mime,
                            filename.as_deref(),
                            &bytes,
                        )?;
                        continue;
                    }
                    let (status, mime, response) = route(app, method, target, body);
                    if status == 200 && target.split('?').next() == Some("/preview") {
                        write_preview_response(&mut stream, response.as_bytes())?;
                    } else {
                        write_response(&mut stream, status, mime, &response)?;
                    }
                }
                Err(_) => continue,
            }
        }
        Ok(())
    }
}

fn valid_host(host: &str, address: SocketAddr) -> bool {
    host.eq_ignore_ascii_case(&address.to_string())
        || host.eq_ignore_ascii_case(&format!("localhost:{}", address.port()))
}

fn valid_origin(origin: &str, host: &str) -> bool {
    origin.eq_ignore_ascii_case(&format!("http://{host}"))
}

fn write_response(
    stream: &mut impl Write,
    status: u16,
    mime: &str,
    body: &str,
) -> std::io::Result<()> {
    let response = format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'self'; style-src 'unsafe-inline'; script-src 'unsafe-inline'\r\nConnection: close\r\n\r\n",
        reason(status),
        body.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.write_all(body.as_bytes())
}

fn write_preview_response(stream: &mut impl Write, body: &[u8]) -> std::io::Result<()> {
    let headers = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nX-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\nContent-Security-Policy: default-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'; style-src 'unsafe-inline'; img-src data:; font-src data:\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(headers.as_bytes())?;
    stream.write_all(body)
}

fn write_binary_response(
    stream: &mut impl Write,
    status: u16,
    mime: &str,
    filename: Option<&str>,
    body: &[u8],
) -> std::io::Result<()> {
    let disposition = filename
        .map(|filename| {
            let safe_filename = filename
                .chars()
                .map(|character| {
                    if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
                        character
                    } else {
                        '_'
                    }
                })
                .collect::<String>();
            format!("Content-Disposition: attachment; filename=\"{safe_filename}\"\r\n")
        })
        .unwrap_or_default();
    let headers = format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\n{disposition}X-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\nContent-Security-Policy: default-src 'none'\r\nConnection: close\r\n\r\n",
        reason(status),
        body.len()
    );
    stream.write_all(headers.as_bytes())?;
    stream.write_all(body)
}

fn add_download_navigation(html: String) -> String {
    let navigation = "<nav aria-label=\"Compile downloads\" style=\"padding:.6em;background:#eef3f8\"><a href=\"/downloads\">Download latest compiled EPUB, DOCX, or PDF</a></nav>";
    if let Some((before, after)) = html.split_once("<body>") {
        format!("{before}<body>{navigation}{after}")
    } else if let Some((before, after)) = html.split_once("<body ") {
        if let Some((attributes, content)) = after.split_once('>') {
            format!("{before}<body {attributes}>{navigation}{content}")
        } else {
            html
        }
    } else {
        format!("{navigation}{html}")
    }
}

struct DownloadArtifact {
    filename: String,
    mime: &'static str,
    path: std::path::PathBuf,
}

struct DownloadArtifactResponse {
    filename: String,
    mime: &'static str,
    bytes: Vec<u8>,
}

fn downloadable_compile_artifacts(app: &WriterApp) -> Result<Vec<DownloadArtifact>, String> {
    let Some(record) = app
        .list_compile_results()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|record| record.succeeded)
    else {
        return Ok(Vec::new());
    };
    let root = std::fs::canonicalize(app.project_root())
        .map_err(|error| format!("resolve project directory: {error}"))?;
    let exports = root.join("exports");
    let exports = match std::fs::canonicalize(exports) {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("resolve project exports directory: {error}")),
    };
    let mut artifacts = Vec::new();
    for output in record.outputs {
        let relative = std::path::Path::new(&output);
        if relative.is_absolute()
            || relative.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::ParentDir | std::path::Component::Prefix(_)
                )
            })
        {
            continue;
        }
        let Some(extension) = relative.extension().and_then(|value| value.to_str()) else {
            continue;
        };
        let mime = match extension.to_ascii_lowercase().as_str() {
            "epub" => "application/epub+zip",
            "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "pdf" => "application/pdf",
            _ => continue,
        };
        let path = std::fs::canonicalize(root.join(relative));
        let Ok(path) = path else { continue };
        if !path.is_file() || !path.starts_with(&exports) {
            continue;
        }
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("manuscript")
            .to_owned();
        artifacts.push(DownloadArtifact {
            filename,
            mime,
            path,
        });
    }
    Ok(artifacts)
}

fn download_compile_artifact(
    app: &WriterApp,
    index: &str,
) -> Result<Option<DownloadArtifactResponse>, String> {
    let Ok(index) = index.parse::<usize>() else {
        return Ok(None);
    };
    let entries = downloadable_compile_artifacts(app)?;
    let Some(entry) = entries.get(index) else {
        return Ok(None);
    };
    let bytes =
        std::fs::read(&entry.path).map_err(|error| format!("read Compile artifact: {error}"))?;
    Ok(Some(DownloadArtifactResponse {
        filename: entry.filename.clone(),
        mime: entry.mime,
        bytes,
    }))
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        _ => "Internal Server Error",
    }
}

fn route(app: &mut WriterApp, method: &str, path: &str, body: &str) -> (u16, &'static str, String) {
    let path = path.split('?').next().unwrap_or(path);
    if method == "GET" && path == "/" {
        return (200, "text/html; charset=utf-8", PAGE.into());
    }
    if method == "GET" && path == "/preview" {
        return match app.compile_artifact(story_document::CompileFormat::Html, false) {
            Ok(artifact) => match String::from_utf8(artifact.bytes) {
                Ok(html) => (
                    200,
                    "text/html; charset=utf-8",
                    add_download_navigation(html),
                ),
                Err(error) => (
                    500,
                    "text/plain; charset=utf-8",
                    format!("HTML renderer returned invalid UTF-8: {error}"),
                ),
            },
            Err(error) => (
                500,
                "text/plain; charset=utf-8",
                format!("Compile preview failed: {error}"),
            ),
        };
    }
    if method == "GET" && path == "/downloads" {
        return match downloadable_compile_artifacts(app) {
            Ok(artifacts) => {
                let links = artifacts
                    .iter()
                    .enumerate()
                    .map(|(index, entry)| {
                        format!(
                            "<li><a href=\"/downloads/{index}\">{}</a></li>",
                            escape_html(&entry.filename)
                        )
                    })
                    .collect::<String>();
                let links = if links.is_empty() {
                    "<li>No supported EPUB, DOCX, or PDF downloads are available from the latest successful Compile run.</li>".to_owned()
                } else {
                    links
                };
                let page = format!(
                    "<!doctype html><html><meta charset=\"utf-8\"><title>Compile downloads</title><h1>Compile downloads</h1><ul>{links}</ul><a href=\"/\">Back to Writer's IDE</a></html>"
                );
                (200, "text/html; charset=utf-8", page)
            }
            Err(error) => (500, "text/plain; charset=utf-8", error),
        };
    }
    if method == "POST" && path == "/api/scenes" {
        return match serde_json::from_str::<CreateSceneRequest>(body) {
            Ok(request) => match uuid::Uuid::from_str(&request.chapter_id)
                .map(story_core::NodeId)
                .map_err(|error| error.to_string())
                .and_then(|chapter| {
                    app.create_scene(chapter, &request.title, request.position)
                        .map_err(|error| error.to_string())
                }) {
                Ok(id) => (
                    201,
                    "application/json; charset=utf-8",
                    serde_json::json!({"id":id}).to_string(),
                ),
                Err(error) => api_error(error),
            },
            Err(error) => api_error(error.to_string()),
        };
    }
    if method == "POST" && path == "/api/chapters" {
        return match serde_json::from_str::<CreateChapterRequest>(body) {
            Ok(request) => {
                let parent = match request.parent_id {
                    Some(parent) => match uuid::Uuid::from_str(&parent) {
                        Ok(parent) => Some(story_core::NodeId(parent)),
                        Err(error) => return api_error(error.to_string()),
                    },
                    None => None,
                };
                match app.create_chapter(parent, &request.title, request.position) {
                    Ok(id) => (
                        201,
                        "application/json; charset=utf-8",
                        serde_json::json!({"id":id}).to_string(),
                    ),
                    Err(error) => api_error(error.to_string()),
                }
            }
            Err(error) => api_error(error.to_string()),
        };
    }
    if method == "GET" && path == "/api/plan" {
        return match plan_json(app) {
            Ok(body) => (200, "application/json; charset=utf-8", body),
            Err(error) => api_error(error),
        };
    }
    if method == "POST" && path == "/api/issues" {
        return match serde_json::from_str::<CreateIssueRequest>(body) {
            Ok(request) => {
                let scene = match request.scene_id.as_deref() {
                    Some(value) => match uuid::Uuid::from_str(value) {
                        Ok(id) => {
                            let id = story_core::SceneId(id);
                            if let Err(error) = app.get_scene(id) {
                                return api_error(error.to_string());
                            }
                            Some(id)
                        }
                        Err(error) => return api_error(error.to_string()),
                    },
                    None => None,
                };
                match app.create_issue(request.title.trim()) {
                    Ok(id) => {
                        if let Some(scene) = scene
                            && let Err(error) = app.link_issue(scene, id)
                        {
                            return api_error(error.to_string());
                        }
                        (
                            201,
                            "application/json; charset=utf-8",
                            serde_json::json!({"created":true,"id":id}).to_string(),
                        )
                    }
                    Err(error) => api_error(error.to_string()),
                }
            }
            Err(error) => api_error(error.to_string()),
        };
    }
    if let Some(id) = path.strip_prefix("/api/issues/")
        && let Some(id) = id.strip_suffix("/status")
        && method == "POST"
    {
        let id = match uuid::Uuid::from_str(id) {
            Ok(id) => story_core::IssueId(id),
            Err(error) => return api_error(error.to_string()),
        };
        return match serde_json::from_str::<IssueStatusRequest>(body) {
            Ok(request) => match app.set_issue_status(id, &request.status) {
                Ok(()) => (
                    200,
                    "application/json; charset=utf-8",
                    serde_json::json!({"saved":true,"id":id}).to_string(),
                ),
                Err(error) => api_error(error.to_string()),
            },
            Err(error) => api_error(error.to_string()),
        };
    }
    if let Some(id) = path.strip_prefix("/api/events/") {
        let (id, action) = id.split_once('/').unwrap_or((id, ""));
        if method == "POST" && action == "time" {
            let id = match uuid::Uuid::from_str(id) {
                Ok(id) => story_core::EventId(id),
                Err(error) => return api_error(error.to_string()),
            };
            return match serde_json::from_str::<EventTimeUpdateRequest>(body) {
                Ok(request) => match parse_story_time(&request.start)
                    .and_then(|start| parse_story_time(&request.end).map(|end| (start, end)))
                {
                    Ok((start, end)) => {
                        match app.update_story_event_time(id, start, end, &request.source_time) {
                            Ok(()) => (
                                200,
                                "application/json; charset=utf-8",
                                serde_json::json!({"saved":true,"id":id}).to_string(),
                            ),
                            Err(error) => api_error(error.to_string()),
                        }
                    }
                    Err(error) => api_error(error),
                },
                Err(error) => api_error(error.to_string()),
            };
        }
    }
    if let Some(id) = path.strip_prefix("/api/scenes/") {
        let (id, action) = id.split_once('/').unwrap_or((id, ""));
        let id = match uuid::Uuid::from_str(id) {
            Ok(id) => story_core::SceneId(id),
            Err(error) => return api_error(error.to_string()),
        };
        if method == "GET" && action.is_empty() {
            return match scene_json(app, id) {
                Ok(body) => (200, "application/json; charset=utf-8", body),
                Err(error) => api_error(error),
            };
        }
        if method == "POST" && action == "time" {
            return match serde_json::from_str::<SceneTimeUpdateRequest>(body) {
                Ok(request) => match parse_story_time(&request.start)
                    .and_then(|start| parse_story_time(&request.end).map(|end| (start, end)))
                {
                    Ok((start, end)) => match app.get_scene(id) {
                        Ok(mut record) => {
                            record.scene.start_time = start;
                            record.scene.end_time = end;
                            match app.update_scene_metadata(&record.scene) {
                                Ok(()) => (
                                    200,
                                    "application/json; charset=utf-8",
                                    serde_json::json!({"saved":true,"id":id}).to_string(),
                                ),
                                Err(error) => api_error(error.to_string()),
                            }
                        }
                        Err(error) => api_error(error.to_string()),
                    },
                    Err(error) => api_error(error),
                },
                Err(error) => api_error(error.to_string()),
            };
        }
        if method == "POST" && action == "planning" {
            return match serde_json::from_str::<PlanningUpdateRequest>(body) {
                Ok(update) => match app.get_scene(id) {
                    Ok(mut record) => {
                        let status = match update.status.as_deref() {
                            Some(value) => match parse_status(value) {
                                Some(status) => Some(status),
                                None => return api_error(format!("unknown scene status: {value}")),
                            },
                            None => None,
                        };
                        if let Some(title) = update.title {
                            record.scene.title = title;
                        }
                        if let Some(synopsis) = update.synopsis {
                            record.scene.synopsis = synopsis;
                        }
                        if let Some(status) = status {
                            record.scene.status = status;
                        }
                        match app.update_scene_metadata(&record.scene) {
                            Ok(()) => (
                                200,
                                "application/json; charset=utf-8",
                                serde_json::json!({"saved":true,"id":id}).to_string(),
                            ),
                            Err(error) => api_error(error.to_string()),
                        }
                    }
                    Err(error) => api_error(error.to_string()),
                },
                Err(error) => api_error(error.to_string()),
            };
        }
        if method == "POST" && action == "move" {
            return match serde_json::from_str::<MoveSceneRequest>(body) {
                Ok(request) => match uuid::Uuid::from_str(&request.chapter_id)
                    .map(story_core::NodeId)
                    .map_err(|error| error.to_string())
                    .and_then(|chapter| {
                        app.move_scene(id, chapter, request.position)
                            .map_err(|error| error.to_string())
                    }) {
                    Ok(()) => (
                        200,
                        "application/json; charset=utf-8",
                        serde_json::json!({"moved":true,"id":id}).to_string(),
                    ),
                    Err(error) => api_error(error),
                },
                Err(error) => api_error(error.to_string()),
            };
        }
    }
    if method != "GET" && method != "POST" {
        return (
            405,
            "text/plain; charset=utf-8",
            "Unsupported method".into(),
        );
    }
    (404, "text/plain; charset=utf-8", "Not found".into())
}

fn parse_story_time(value: &str) -> Result<story_core::StoryTime, String> {
    use story_core::{StoryTime, TimePrecision};
    if value == "?" {
        return Ok(StoryTime::unknown());
    }
    if let Some(value) = value.strip_prefix('~') {
        let parts = value.split(',').collect::<Vec<_>>();
        if parts.len() != 3 {
            return Err("approximate time must use ~NOMINAL,LOWER,UPPER".into());
        }
        return StoryTime::new(
            Some(parts[0].parse().map_err(|_| "invalid nominal tick")?),
            Some(parts[1].parse().map_err(|_| "invalid lower tick")?),
            Some(parts[2].parse().map_err(|_| "invalid upper tick")?),
            TimePrecision::Approximate,
        )
        .map_err(|error| error.to_string());
    }
    if let Some((lower, upper)) = value.split_once("..") {
        return StoryTime::new(
            None,
            Some(lower.parse().map_err(|_| "invalid lower tick")?),
            Some(upper.parse().map_err(|_| "invalid upper tick")?),
            TimePrecision::Bounded,
        )
        .map_err(|error| error.to_string());
    }
    value.parse().map(StoryTime::exact).map_err(|_| {
        "invalid tick; use an integer, ?, ~nominal,lower,upper, or lower..upper".into()
    })
}

fn api_error(message: String) -> (u16, &'static str, String) {
    (
        400,
        "application/json; charset=utf-8",
        serde_json::json!({"error":message}).to_string(),
    )
}

fn parse_status(value: &str) -> Option<story_core::SceneStatus> {
    use story_core::SceneStatus::*;
    Some(match value {
        "idea" => Idea,
        "outlined" => Outlined,
        "ready_to_draft" => ReadyToDraft,
        "drafting" => Drafting,
        "draft_complete" => DraftComplete,
        "revising" => Revising,
        "complete" => Complete,
        "archived" => Archived,
        _ => return None,
    })
}

fn plan_json(app: &WriterApp) -> Result<String, String> {
    let project = app.get_project().map_err(|error| error.to_string())?;
    let groups = app
        .list_structural_nodes()
        .map_err(|error| error.to_string())?;
    let scenes = app.list_scenes(false).map_err(|error| error.to_string())?;
    let analytics = app.analytics_dataset().map_err(|error| error.to_string())?;
    let chars = app.list_characters().map_err(|error| error.to_string())?;
    let plots = app.list_plotlines().map_err(|error| error.to_string())?;
    let locations = app.list_locations().map_err(|error| error.to_string())?;
    let events = app.list_events().map_err(|error| error.to_string())?;
    let issues = app.list_issues().map_err(|error| error.to_string())?;
    let research = app
        .list_research_items()
        .map_err(|error| error.to_string())?;
    let claims = app
        .list_approved_knowledge_claims()
        .map_err(|error| error.to_string())?;
    let by_scene = analytics
        .into_iter()
        .map(|row| (row.scene_id.clone(), row))
        .collect::<std::collections::BTreeMap<_, _>>();
    let depth_of = |node: &story_storage::StructuralNodeSummary| {
        let mut depth = 0usize;
        let mut parent = node.parent_id;
        while let Some(id) = parent {
            depth += 1;
            parent = groups
                .iter()
                .find(|candidate| candidate.id == id)
                .and_then(|candidate| candidate.parent_id);
        }
        depth
    };
    let nodes = groups.iter().map(|node| serde_json::json!({"id":node.id,"parent_id":node.parent_id,"kind":node.kind,"title":node.title,"depth":depth_of(node)})).collect::<Vec<_>>();
    let scene_rows = scenes.iter().map(|row| {
        let analysis = by_scene.get(&row.scene.id.to_string());
        let chapter = row.chapter_id.and_then(|id| groups.iter().find(|node| node.id == id)).map(|node| node.title.clone());
        serde_json::json!({"id":row.scene.id,"title":row.scene.title,"synopsis":row.scene.synopsis,"status":row.scene.status.as_str(),"chapter":chapter,"chapter_id":row.chapter_id,"position":row.position,"actual_words":row.actual_words,"target_words":row.scene.target_words,"pov":row.scene.pov_character,"start_time":row.scene.start_time,"end_time":row.scene.end_time,"characters":analysis.map(|row| row.characters.iter().map(|character| serde_json::json!({"id":character.id,"name":character.name,"role":character.role})).collect::<Vec<_>>()).unwrap_or_default(),"plotlines":analysis.map(|row| row.plotlines.iter().map(|plot| serde_json::json!({"id":plot.id,"name":plot.name})).collect::<Vec<_>>()).unwrap_or_default()})
    }).collect::<Vec<_>>();
    let mut edges = by_scene.values().flat_map(|scene| {
        scene.characters.iter().map(|character| serde_json::json!({"from":scene.scene_id,"to":character.id,"kind":"character-participation","role":character.role}))
            .chain(scene.plotlines.iter().map(|plotline| serde_json::json!({"from":scene.scene_id,"to":plotline.id,"kind":"plotline-membership"})))
    }).collect::<Vec<_>>();
    for scene in &scenes {
        if let Some(chapter) = scene.chapter_id {
            edges.push(serde_json::json!({"from":chapter,"to":scene.scene.id,"kind":"manuscript-placement"}));
        }
        if let Some(location) = scene.scene.start_location {
            edges.push(serde_json::json!({"from":scene.scene.id,"to":location,"kind":"starts-at"}));
        }
        if let Some(location) = scene.scene.end_location {
            edges.push(serde_json::json!({"from":scene.scene.id,"to":location,"kind":"ends-at"}));
        }
        for (event, relation) in app
            .scene_event_links(scene.scene.id)
            .map_err(|error| error.to_string())?
        {
            edges.push(serde_json::json!({"from":scene.scene.id,"to":event,"kind":"story-event","relation":relation}));
        }
        for link in app
            .outgoing_links(story_core::EntityRef::Scene(scene.scene.id))
            .map_err(|error| error.to_string())?
        {
            edges.push(serde_json::json!({"from":link.source.id(),"to":link.target.id(),"kind":link.kind,"label":link.label}));
        }
    }
    for claim in &claims {
        edges.push(serde_json::json!({"from":claim.source_id,"to":claim.id,"kind":"approved-knowledge","source_kind":claim.source_kind,"evidence":claim.evidence_quote}));
    }
    Ok(serde_json::json!({"project":{"title":project.title,"book_type":project.book_type,"target_words":project.target_words},"groups":nodes,"scenes":scene_rows,"claims":claims.iter().map(|claim|serde_json::json!({"id":claim.id,"kind":claim.kind,"name":claim.name,"claim":claim.claim,"source_kind":claim.source_kind,"source_id":claim.source_id,"evidence_quote":claim.evidence_quote,"evidence_start_byte":claim.evidence_start_byte,"evidence_end_byte":claim.evidence_end_byte})).collect::<Vec<_>>(),"characters":chars.iter().map(|item|serde_json::json!({"id":item.id,"name":item.name,"scene_count":item.scene_count})).collect::<Vec<_>>(),"plotlines":plots.iter().map(|item|serde_json::json!({"id":item.id,"name":item.name,"scene_count":item.scene_count})).collect::<Vec<_>>(),"locations":locations.iter().map(|item|serde_json::json!({"id":item.id,"name":item.name})).collect::<Vec<_>>(),"events":events.iter().map(|item|serde_json::json!({"id":item.id,"title":item.title,"start":item.start,"end":item.end,"source_time":item.source_time})).collect::<Vec<_>>(),"issues":issues.iter().map(|item|serde_json::json!({"id":item.id,"title":item.title,"status":item.status,"scene_count":item.scene_count})).collect::<Vec<_>>(),"research":research.iter().map(|item|serde_json::json!({"id":item.id,"title":item.title,"media_type":item.media_type})).collect::<Vec<_>>(),"edges":edges}).to_string())
}

fn scene_json(app: &WriterApp, id: story_core::SceneId) -> Result<String, String> {
    let row = app.get_scene(id).map_err(|error| error.to_string())?;
    let (text, hash) = app.load_scene_text(id).map_err(|error| error.to_string())?;
    Ok(serde_json::json!({"id":id,"title":row.scene.title,"synopsis":row.scene.synopsis,"status":row.scene.status.as_str(),"pov":row.scene.pov_character,"actual_words":row.actual_words,"content_hash":hash,"text":text}).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn browser_routes_expose_shared_plan_and_read_only_scene_text() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Novel"), "Browser Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter One", 0).unwrap();
        let scene = app.create_scene(chapter, "Opening", 0).unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "Review only.")
            .unwrap();
        let proposal = story_ollama::attach_provenance(
            "scene",
            &scene.to_string(),
            "Review only.",
            "fixture-model",
            story_ollama::ExtractionResult {
                claims: vec![story_ollama::CandidateClaim {
                    kind: "event".into(),
                    name: "Review begins".into(),
                    claim: "A review begins.".into(),
                    evidence_quote: "Review only.".into(),
                    approximate_story_time: None,
                }],
            },
        );
        let claim_id = app.approve_extraction_claim(&proposal, 0).unwrap();
        let (status, mime, plan) = route(&mut app, "GET", "/api/plan", "");
        assert_eq!(status, 200);
        assert!(mime.starts_with("application/json"));
        assert!(plan.contains("Browser Novel") && plan.contains("Chapter One"));
        assert!(plan.contains("manuscript-placement"));
        assert!(plan.contains(&claim_id.to_string()));
        assert!(plan.contains("approved-knowledge"));
        assert!(plan.contains("Review only."));
        let (status, mime, page) = route(&mut app, "GET", "/", "");
        assert_eq!(status, 200);
        assert!(mime.starts_with("text/html"));
        assert!(page.contains("<svg id=\"graph\""));
        assert!(page.contains("function drawGraph()"));
        assert!(page.contains("scene-filter-status"));
        assert!(page.contains("scene-filter-character"));
        assert!(page.contains("scene-filter-plot"));
        assert!(page.contains("function renderScenes()"));
        assert!(page.contains("aria-label=\"Story relationship and approved knowledge graph\""));
        assert!(page.contains("scene-start-time"));
        assert!(page.contains("create-issue"));
        assert!(page.contains("data-issue-status"));
        let (status, _, document) = route(&mut app, "GET", &format!("/api/scenes/{scene}"), "");
        assert_eq!(status, 200);
        assert!(document.contains("Review only."));
        assert_eq!(route(&mut app, "GET", "/api/scenes/not-a-uuid", "").0, 400);
        assert_eq!(route(&mut app, "GET", "/unknown", "").0, 404);
    }

    #[test]
    fn browser_planning_edits_use_services_and_never_write_scene_prose() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Novel"), "Browser Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter One", 0).unwrap();
        let chapter_two = app.create_chapter(None, "Chapter Two", 1).unwrap();
        let scene = app.create_scene(chapter, "Opening", 0).unwrap();
        let time_body = serde_json::json!({"start":"~10,8,12","end":"20"}).to_string();
        assert_eq!(
            route(
                &mut app,
                "POST",
                &format!("/api/scenes/{scene}/time"),
                &time_body
            )
            .0,
            200
        );
        assert_eq!(
            app.get_scene(scene).unwrap().scene.start_time,
            story_core::StoryTime::new(
                Some(10),
                Some(8),
                Some(12),
                story_core::TimePrecision::Approximate
            )
            .unwrap()
        );
        let invalid_time = serde_json::json!({"start":"10..5","end":"20"}).to_string();
        assert_eq!(
            route(
                &mut app,
                "POST",
                &format!("/api/scenes/{scene}/time"),
                &invalid_time
            )
            .0,
            400
        );
        let issue_body =
            serde_json::json!({"title":"Check the opening","scene_id":scene}).to_string();
        let (status, _, created) = route(&mut app, "POST", "/api/issues", &issue_body);
        assert_eq!(status, 201);
        let issue_id = serde_json::from_str::<serde_json::Value>(&created).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let status_body = serde_json::json!({"status":"in_progress"}).to_string();
        assert_eq!(
            route(
                &mut app,
                "POST",
                &format!("/api/issues/{issue_id}/status"),
                &status_body
            )
            .0,
            200
        );
        assert!(
            app.list_issues()
                .unwrap()
                .iter()
                .any(|issue| { issue.id.to_string() == issue_id && issue.status == "in_progress" })
        );
        let event_id = story_core::EventId::new();
        app.add_story_event(&story_core::StoryEvent {
            id: event_id,
            title: "The bridge falls".into(),
            description: "A key turning point".into(),
            start: story_core::StoryTime::exact(1),
            end: story_core::StoryTime::exact(2),
            source_time_text: "day one".into(),
        })
        .unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "Canonical prose.")
            .unwrap();
        let body = serde_json::json!({"title":"Renamed Opening","synopsis":"A changed plan","status":"outlined"}).to_string();
        assert_eq!(
            route(
                &mut app,
                "POST",
                &format!("/api/scenes/{scene}/planning"),
                &body
            )
            .0,
            200
        );
        let body = serde_json::json!({"chapter_id":chapter_two,"position":0}).to_string();
        assert_eq!(
            route(
                &mut app,
                "POST",
                &format!("/api/scenes/{scene}/move"),
                &body
            )
            .0,
            200
        );
        let row = app.get_scene(scene).unwrap();
        assert_eq!(row.scene.synopsis, "A changed plan");
        assert_eq!(row.scene.title, "Renamed Opening");
        assert_eq!(row.scene.status, story_core::SceneStatus::Outlined);
        assert_eq!(row.chapter_id, Some(chapter_two));
        assert_eq!(app.load_scene_text(scene).unwrap().0, "Canonical prose.");
        let body =
            serde_json::json!({"start":"~6,4,8","end":"9..12","source_time":"during the storm"})
                .to_string();
        assert_eq!(
            route(
                &mut app,
                "POST",
                &format!("/api/events/{event_id}/time"),
                &body
            )
            .0,
            200
        );
        let event = app
            .list_events()
            .unwrap()
            .into_iter()
            .find(|event| event.id == event_id)
            .unwrap();
        assert_eq!(event.title, "The bridge falls");
        assert_eq!(event.start.lower, Some(4));
        assert_eq!(event.start.upper, Some(8));
        assert_eq!(event.end.lower, Some(9));
        assert_eq!(event.end.upper, Some(12));
        assert_eq!(event.source_time, "during the storm");
        assert!(PAGE.contains("data-event"));
        let body =
            serde_json::json!({"title":"Chapter Three","parent_id":null,"position":2}).to_string();
        assert_eq!(route(&mut app, "POST", "/api/chapters", &body).0, 201);
        let body = serde_json::json!({"chapter_id":chapter_two,"position":1,"title":"Aftermath"})
            .to_string();
        assert_eq!(route(&mut app, "POST", "/api/scenes", &body).0, 201);
        assert_eq!(app.list_scenes(false).unwrap().len(), 2);
        assert_eq!(
            route(&mut app, "POST", &format!("/api/scenes/{scene}"), "{}").0,
            404
        );
        assert_eq!(route(&mut app, "PUT", "/api/scenes", "{}").0, 405);
    }

    #[test]
    fn browser_server_binds_only_loopback() {
        assert!(BrowserServer::loopback_address(7878).ip().is_loopback());
        let address = BrowserServer::loopback_address(7878);
        assert!(valid_host("127.0.0.1:7878", address));
        assert!(valid_host("localhost:7878", address));
        assert!(!valid_host("evil.example:7878", address));
        assert!(valid_origin("http://localhost:7878", "localhost:7878"));
        assert!(!valid_origin("https://evil.example", "localhost:7878"));
    }

    #[test]
    fn browser_compile_preview_renders_html_without_mutating_manuscript() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Novel"), "Browser Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter One", 0).unwrap();
        let scene = app.create_scene(chapter, "Opening", 0).unwrap();
        let prose = "Preview text remains canonical.";
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), prose).unwrap();

        let (status, mime, html) = route(&mut app, "GET", "/preview", "");
        assert_eq!(status, 200);
        assert_eq!(mime, "text/html; charset=utf-8");
        assert!(html.contains("Preview text remains canonical."));
        assert!(html.contains("href=\"/downloads\""));
        assert!(PAGE.contains("href=\"/preview\""));
        assert_eq!(app.load_scene_text(scene).unwrap().0, prose);

        let mut response = Vec::new();
        write_preview_response(&mut response, html.as_bytes()).unwrap();
        let response = String::from_utf8(response).unwrap();
        assert!(response.contains("Content-Security-Policy: default-src 'none'"));
        assert!(!response.contains("script-src 'unsafe-inline'"));
        assert!(response.ends_with(&html));
    }

    #[test]
    fn compile_download_page_serves_only_latest_managed_artifacts() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Novel"), "Browser Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter One", 0).unwrap();
        let scene = app.create_scene(chapter, "Opening", 0).unwrap();
        let prose = "Download text remains canonical.";
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), prose).unwrap();
        app.create_builtin_compile_configuration("ebook", "ebook")
            .unwrap();
        let build = app.compile_configuration("ebook", false).unwrap();
        assert!(build.outputs.iter().any(|output| output.ends_with(".epub")));

        let (status, mime, page) = route(&mut app, "GET", "/downloads", "");
        assert_eq!(status, 200);
        assert_eq!(mime, "text/html; charset=utf-8");
        assert!(page.contains("/downloads/0"));
        let artifact = download_compile_artifact(&app, "0")
            .unwrap()
            .expect("compiled EPUB is downloadable");
        assert_eq!(artifact.mime, "application/epub+zip");
        assert!(artifact.filename.ends_with(".epub"));
        assert!(artifact.bytes.starts_with(b"PK"));
        assert_eq!(app.load_scene_text(scene).unwrap().0, prose);

        let mut response = Vec::new();
        write_binary_response(
            &mut response,
            200,
            artifact.mime,
            Some(&artifact.filename),
            &artifact.bytes,
        )
        .unwrap();
        let headers = String::from_utf8_lossy(
            &response[..response
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .unwrap()],
        );
        assert!(headers.contains("Content-Disposition: attachment;"));
        assert!(headers.contains("application/epub+zip"));
        assert!(response.ends_with(&artifact.bytes));

        let mut unsafe_name_response = Vec::new();
        write_binary_response(
            &mut unsafe_name_response,
            200,
            "application/pdf",
            Some("safe.pdf\r\nX-Injected: yes"),
            b"pdf",
        )
        .unwrap();
        let headers = String::from_utf8_lossy(&unsafe_name_response);
        assert!(!headers.contains("\r\nX-Injected:"));
    }
}
