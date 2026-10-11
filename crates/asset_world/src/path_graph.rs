//! AI path nodes shipped in a zone's `PathData`, copied out of the zone arena.

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PathGraph {
    pub nodes: Vec<PathNode>,
    pub links: Vec<PathLink>,
    /// Links whose target index fell outside `nodes`; they are not kept.
    pub dropped_links: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PathNode {
    /// Raw node type as stored in the zone.
    pub node_type: u32,
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub targetname: Option<String>,
    pub script_noteworthy: Option<String>,
    pub first_link: u32,
    pub link_count: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PathLink {
    pub target: u32,
    pub distance: f32,
}

impl PathGraph {
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn links_of(&self, node: usize) -> &[PathLink] {
        let Some(n) = self.nodes.get(node) else {
            return &[];
        };
        let start = n.first_link as usize;
        &self.links[start..start + n.link_count as usize]
    }

    pub fn report_line(&self) -> String {
        format!(
            "path graph: {} nodes, {} links ({} dropped)",
            self.nodes.len(),
            self.links.len(),
            self.dropped_links
        )
    }
}

const NODE_TYPE: usize = 0;
const NODE_TARGETNAME: usize = 6;
const NODE_SCRIPT_NOTEWORTHY: usize = 10;
const NODE_ORIGIN: usize = 20;
const NODE_YAW: usize = 32;
const NODE_LINK_COUNT: usize = 56;
const LINK_DISTANCE: usize = 0;
const LINK_TARGET: usize = 4;
const LINK_STRIDE: usize = fastfile_iw5::size::PATH_LINK;

pub fn capture_path_graph_iw5(
    s: &fastfile_iw5::ZoneStream<'_>,
    strings: &fastfile_iw5::ScriptStrings,
    geometry: fastfile_iw5::PathDataGeometry,
) -> PathGraph {
    let mut graph = PathGraph::default();
    let Some(nodes) = geometry.nodes else {
        return graph;
    };
    let named = |node, off| {
        let index = s.u16_at(node, off).ok()?;
        if index == 0 {
            return None;
        }
        strings
            .get(s, index)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
    };
    let f32_at = |p, off| s.f32_at(p, off).unwrap_or(0.0);
    graph.nodes.reserve(geometry.node_count);
    for i in 0..geometry.node_count {
        let node = nodes.at(i * geometry.node_stride);
        let first_link = graph.links.len() as u32;
        let link_count = s.u16_at(node, NODE_LINK_COUNT).unwrap_or(0) as usize;
        if let Ok(fastfile_iw5::ZonePtr::Offset(p)) = s.ptr_at(node, geometry.link_field) {
            let links = s.resolve_alias(p);
            for k in 0..link_count {
                let link = links.at(k * LINK_STRIDE);
                let target = s.u16_at(link, LINK_TARGET).unwrap_or(u16::MAX) as usize;
                if target >= geometry.node_count {
                    graph.dropped_links += 1;
                    continue;
                }
                graph.links.push(PathLink {
                    target: target as u32,
                    distance: f32_at(link, LINK_DISTANCE),
                });
            }
        }
        graph.nodes.push(PathNode {
            node_type: s.u32_at(node, NODE_TYPE).unwrap_or(0),
            origin: [
                f32_at(node, NODE_ORIGIN),
                f32_at(node, NODE_ORIGIN + 4),
                f32_at(node, NODE_ORIGIN + 8),
            ],
            angles: [0.0, f32_at(node, NODE_YAW), 0.0],
            targetname: named(node, NODE_TARGETNAME),
            script_noteworthy: named(node, NODE_SCRIPT_NOTEWORTHY),
            first_link,
            link_count: graph.links.len() as u32 - first_link,
        });
    }
    graph
}
