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

const PAGE: &str = r#"<!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width">
<title>Writer's IDE</title><style>
body{font:16px system-ui;margin:0;color:#20242a;background:#f3f5f7}header{padding:1rem 2rem;background:#24354a;color:white}main{display:grid;grid-template-columns:minmax(18rem,32%) 1fr;gap:1rem;padding:1rem;max-width:1400px;margin:auto}section{background:white;border:1px solid #ccd2d9;border-radius:.4rem;padding:1rem}button{margin:.3rem 0;padding:.55rem;cursor:pointer}button.scene{display:block;width:100%;text-align:left;border:0;border-bottom:1px solid #e5e8eb;background:white;padding:.7rem}button.scene:hover{background:#eef3f8}textarea, input, select{box-sizing:border-box;width:100%;padding:.5rem;margin:.2rem 0  .5rem}pre{white-space:pre-wrap;overflow-wrap:anywhere;font:inherit;line-height:1.6}small{color:#52606d}.notice{min-height:1.4rem;color:#234f73}.graph{overflow:auto;border:1px solid #d7dce2;border-radius:.3rem}.graph svg{display:block;width:100%;min-width:760px}.graph line{stroke:#9aa7b4;stroke-width:1.5}.graph rect{rx:6;stroke:#617181;stroke-width:1}.graph text{font:12px system-ui;fill:#17202a}.graph g[data-scene-id]{cursor:pointer}.graph g[data-scene-id]:hover rect{stroke:#1769aa;stroke-width:3}.graph .group rect{fill:#f0f2f5}.graph .scene rect{fill:#e7f1fb}.graph .character rect{fill:#e8f5ec}.graph .plotline rect{fill:#fff2d9}.graph .world rect{fill:#f4eafb}@media(max-width:750px){main{grid-template-columns:1fr}}</style>
<header><h1 id="title">Writer's IDE</h1><p>Local planning graph · manuscript prose remains read-only here</p></header><main>
<section><h2>Scenes</h2><div id="scenes">Loading…</div><h3>Add chapter</h3><form id="create-chapter"><input id="new-chapter-title" required placeholder="Chapter title"><button>Add chapter</button></form><h3>Add scene</h3><form id="create-scene"><select id="new-chapter"></select><input id="new-title" required placeholder="Scene title"><button>Add scene</button></form></section>
<section><h2 id="scene-title">Select a scene</h2><small id="scene-meta"></small><p><label>Scene title<input id="scene-name"></label><label>Planning synopsis<textarea id="synopsis" rows="4"></textarea></label><label>Status<select id="status"><option>idea</option><option>outlined</option><option>ready_to_draft</option><option>drafting</option><option>draft_complete</option><option>revising</option><option>complete</option></select></label><button id="save-planning">Save planning</button></p><p><label>Move to chapter<select id="move-chapter"></select></label><label>Position<input id="move-position" type="number" min="0" value="0"></label><button id="move-scene">Move scene</button></p><p class="notice" id="notice"></p><h3>Read-only manuscript</h3><pre id="prose">Select a scene to inspect its text.</pre></section>
<section><h2>Characters</h2><div id="characters"></div></section><section><h2>Plotlines</h2><div id="plotlines"></div></section><section><h2>Locations and events</h2><div id="world"></div></section><section><h2>Research and issues</h2><div id="work"></div></section><section><h2>Story structure and graph</h2><div id="structure"></div><div class="graph"><svg id="graph" role="img" aria-label="Story relationship graph"></svg></div><div id="edges"></div></section></main>
<script>
let plan,selected;async function api(path,method='GET',data){const r=await fetch(path,{method,headers:data?{'Content-Type':'application/json'}:{},body:data?JSON.stringify(data):undefined});const j=await r.json().catch(()=>({}));if(!r.ok)throw Error(j.error||r.statusText);return j}
async function load(){plan=await api('/api/plan');document.querySelector('#title').textContent=plan.project.title;document.querySelector('#scenes').innerHTML=plan.scenes.map(s=>`<button class="scene" data-id="${s.id}"><b>${esc(s.title)}</b><br><small>${esc(s.chapter||'Unplaced')} · ${s.actual_words} words · ${esc(s.status)}</small><p>${esc(s.synopsis||'')}</p></button>`).join('')||'No scenes yet';document.querySelector('#characters').innerHTML=plan.characters.map(x=>`<p>${esc(x.name)} <small>(${x.scene_count} scenes)</small></p>`).join('')||'No characters';document.querySelector('#plotlines').innerHTML=plan.plotlines.map(x=>`<p>${esc(x.name)} <small>(${x.scene_count} scenes)</small></p>`).join('')||'No plotlines';document.querySelector('#world').innerHTML=[...plan.locations.map(x=>`<p>Location · ${esc(x.name)}</p>`),...plan.events.map(x=>`<p><button data-event="${esc(x.id)}" data-start="${esc(timeLabel(x.start))}" data-end="${esc(timeLabel(x.end))}" data-source="${esc(x.source_time)}">Edit time · ${esc(x.title)}</button> <small>${esc(x.source_time)} · ${esc(timeLabel(x.start))} → ${esc(timeLabel(x.end))}</small></p>`)].join('')||'No locations or events';document.querySelector('#work').innerHTML=[...plan.research.map(x=>`<p>Research · ${esc(x.title)}</p>`),...plan.issues.map(x=>`<p>Issue · ${esc(x.title)} <small>${esc(x.status)}</small></p>`)].join('')||'No research or issues';document.querySelector('#structure').innerHTML=plan.groups.map(x=>`<p>${'　'.repeat(x.depth)}${esc(x.title)} <small>${esc(x.kind)}</small></p>`).join('')||'No structure';drawGraph();document.querySelector('#edges').innerHTML=`<small>${plan.edges.length} stable-ID relationships in the project graph</small>`;const chapters=plan.groups.filter(x=>x.kind==='chapter');for(const id of ['new-chapter','move-chapter'])document.querySelector('#'+id).innerHTML=chapters.map(c=>`<option value="${c.id}">${esc(c.title)}</option>`).join('');document.querySelectorAll('[data-id]').forEach(b=>b.onclick=()=>openScene(b.dataset.id))}
function drawGraph(){const columns=[['group',plan.groups.map(n=>({...n,label:n.title}))],['scene',plan.scenes.map(n=>({...n,label:n.title}))],['character',plan.characters.map(n=>({...n,label:n.name}))],['plotline',plan.plotlines.map(n=>({...n,label:n.name}))],['world',[...plan.locations.map(n=>({...n,label:'Location · '+n.name})),...plan.events.map(n=>({...n,label:'Event · '+n.title}))]]];const nodes=[],positions=new Map();let height=180;columns.forEach(([kind,items],col)=>{items.forEach((n,row)=>{const x=20+col*190,y=24+row*54;const node={id:String(n.id),kind,label:n.label,x,y};nodes.push(node);if(!positions.has(node.id))positions.set(node.id,node);height=Math.max(height,y+52)})});const lines=plan.edges.map(e=>{const a=positions.get(String(e.from)),b=positions.get(String(e.to));return a&&b?`<line x1="${a.x+160}" y1="${a.y+16}" x2="${b.x}" y2="${b.y+16}"><title>${esc(e.kind)}</title></line>`:''}).join('');const cards=nodes.map(n=>`<g class="${n.kind}" ${n.kind==='scene'?`data-scene-id="${esc(n.id)}"`:''}><rect x="${n.x}" y="${n.y}" width="160" height="32"></rect><text x="${n.x+8}" y="${n.y+21}">${esc(n.label.slice(0,22))}</text><title>${esc(n.label)}</title></g>`).join('');const svg=document.querySelector('#graph');svg.setAttribute('viewBox',`0 0 980 ${height}`);svg.setAttribute('height',height);svg.innerHTML=lines+cards;svg.querySelectorAll('[data-scene-id]').forEach(node=>node.onclick=()=>openScene(node.dataset.sceneId))}
async function openScene(id){selected=id;const s=await api('/api/scenes/'+encodeURIComponent(id));document.querySelector('#scene-title').textContent=s.title;document.querySelector('#scene-meta').textContent=`${s.actual_words} words · ${s.status} · POV ${s.pov||'unset'}`;document.querySelector('#scene-name').value=s.title;document.querySelector('#synopsis').value=s.synopsis;document.querySelector('#status').value=s.status;document.querySelector('#prose').textContent=s.text}
document.querySelector('#create-scene').onsubmit=async e=>{e.preventDefault();try{const chapter=document.querySelector('#new-chapter').value;const position=plan.scenes.filter(s=>s.chapter_id===chapter).length;const s=await api('/api/scenes','POST',{chapter_id:chapter,title:document.querySelector('#new-title').value,position});document.querySelector('#new-title').value='';await load();await openScene(s.id);notice('Scene created')}catch(e){notice(e.message)}};
document.querySelector('#create-chapter').onsubmit=async e=>{e.preventDefault();try{await api('/api/chapters','POST',{title:document.querySelector('#new-chapter-title').value,position:plan.groups.filter(g=>g.kind==='chapter').length});document.querySelector('#new-chapter-title').value='';await load();notice('Chapter created')}catch(e){notice(e.message)}};
document.querySelector('#save-planning').onclick=async()=>{if(!selected)return;try{await api('/api/scenes/'+selected+'/planning','POST',{title:document.querySelector('#scene-name').value,synopsis:document.querySelector('#synopsis').value,status:document.querySelector('#status').value});await load();await openScene(selected);notice('Planning metadata saved')}catch(e){notice(e.message)}};
document.querySelector('#move-scene').onclick=async()=>{if(!selected)return;try{await api('/api/scenes/'+selected+'/move','POST',{chapter_id:document.querySelector('#move-chapter').value,position:Number(document.querySelector('#move-position').value)});await load();await openScene(selected);notice('Scene moved; prose and story chronology are unchanged')}catch(e){notice(e.message)}};
function notice(s){document.querySelector('#notice').textContent=s}function esc(s){return String(s).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]))}document.querySelector('#world').addEventListener('click',e=>{const b=e.target.closest('[data-event]');if(b)editEventTime(b)});
function timeLabel(t){if(t.precision==='Unknown')return '?';if(t.precision==='Bounded')return `${t.lower}..${t.upper}`;if(t.precision==='Approximate')return `~${t.nominal},${t.lower},${t.upper}`;return String(t.nominal)}
async function editEventTime(b){const start=prompt('Start: tick, ?, ~nominal,lower,upper, or lower..upper',b.dataset.start);if(start===null)return;const end=prompt('End: tick, ?, ~nominal,lower,upper, or lower..upper',b.dataset.end);if(end===null)return;const source=prompt('Source time label',b.dataset.source);if(source===null)return;try{await api(`/api/events/${encodeURIComponent(b.dataset.event)}/time`,'POST',{start,end,source_time:source});await load();notice('Event chronology saved')}catch(e){notice(e.message)}}
load().catch(e=>notice(e.message));
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
                    let (status, mime, response) = route(app, method, target, body);
                    write_response(&mut stream, status, mime, &response)?;
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
    Ok(serde_json::json!({"project":{"title":project.title,"book_type":project.book_type,"target_words":project.target_words},"groups":nodes,"scenes":scene_rows,"characters":chars.iter().map(|item|serde_json::json!({"id":item.id,"name":item.name,"scene_count":item.scene_count})).collect::<Vec<_>>(),"plotlines":plots.iter().map(|item|serde_json::json!({"id":item.id,"name":item.name,"scene_count":item.scene_count})).collect::<Vec<_>>(),"locations":locations.iter().map(|item|serde_json::json!({"id":item.id,"name":item.name})).collect::<Vec<_>>(),"events":events.iter().map(|item|serde_json::json!({"id":item.id,"title":item.title,"start":item.start,"end":item.end,"source_time":item.source_time})).collect::<Vec<_>>(),"issues":issues.iter().map(|item|serde_json::json!({"id":item.id,"title":item.title,"status":item.status,"scene_count":item.scene_count})).collect::<Vec<_>>(),"research":research.iter().map(|item|serde_json::json!({"id":item.id,"title":item.title,"media_type":item.media_type})).collect::<Vec<_>>(),"edges":edges}).to_string())
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
        let (status, mime, plan) = route(&mut app, "GET", "/api/plan", "");
        assert_eq!(status, 200);
        assert!(mime.starts_with("application/json"));
        assert!(plan.contains("Browser Novel") && plan.contains("Chapter One"));
        assert!(plan.contains("manuscript-placement"));
        let (status, mime, page) = route(&mut app, "GET", "/", "");
        assert_eq!(status, 200);
        assert!(mime.starts_with("text/html"));
        assert!(page.contains("<svg id=\"graph\""));
        assert!(page.contains("function drawGraph()"));
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
}
