use std::collections::BTreeMap;

use crate::ir::Direction;
use serde::{Deserialize, Serialize};

/// A unique trace identifier for a single diagram render pipeline invocation.
///
/// Generated once per `compute_layout` call and threaded through every phase
/// so that decision records can be correlated back to a specific render run.
///
/// This mirrors the `franken_kernel::TraceId` pattern from frankenmermaid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraceId(pub u128);

impl TraceId {
    /// Create a new random trace id (based on monotonic counter and process seed).
    pub fn new() -> Self {
        use std::sync::atomic::AtomicU64;
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let pid = std::process::id() as u128;
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed) as u128;
        Self(
            pid.wrapping_mul(3)
                .wrapping_add(time)
                .wrapping_mul(7)
                .wrapping_add(seq),
        )
    }
}

impl Default for TraceId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TraceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:032x}", self.0)
    }
}

/// A render-agnostic scene description built from Layout.
/// Intermediate representation between layout computation and
/// any render backend (SVG, terminal, canvas).
#[derive(Debug, Clone, Default)]
pub struct RenderScene {
    pub width: f32,
    pub height: f32,
    pub groups: Vec<RenderGroup>,
}

/// A named group of render items (e.g. a subgraph, a node cluster).
#[derive(Debug, Clone)]
pub struct RenderGroup {
    pub label: Option<String>,
    pub items: Vec<RenderItem>,
}

/// A single renderable primitive in the scene.
#[derive(Debug, Clone)]
pub enum RenderItem {
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        rx: Option<f32>,
        fill: Option<String>,
        stroke: Option<String>,
        stroke_width: f32,
    },
    Circle {
        cx: f32,
        cy: f32,
        r: f32,
        fill: Option<String>,
        stroke: Option<String>,
        stroke_width: f32,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        stroke: Option<String>,
        stroke_width: f32,
    },
    Polyline {
        points: Vec<(f32, f32)>,
        stroke: Option<String>,
        stroke_width: f32,
        fill: Option<String>,
    },
    Text {
        x: f32,
        y: f32,
        text: String,
        font_size: f32,
        font_family: Option<String>,
        fill: Option<String>,
        anchor: TextAnchor,
    },
    Path {
        d: String,
        fill: Option<String>,
        stroke: Option<String>,
        stroke_width: f32,
    },
}

/// Text anchor horizontal alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAnchor {
    Start,
    Middle,
    End,
}

/// A single decision record captured during layout computation.
///
/// Each decision records what was decided, why, and the measurable outcome.
/// This is a simplified version of frankenmermaid's `MermaidLayoutDecisionRecord`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionEntry {
    /// The pipeline phase this decision belongs to (e.g. "rank_assignment", "crossing_minimization", "finalize").
    pub phase: String,
    /// Human-readable description of what was decided.
    pub what: String,
    /// Why this decision was made (e.g. "selected Sugiyama algorithm for Flowchart diagram type").
    pub rationale: String,
    /// Optional numeric outcome for regression analysis (e.g. crossing count, edge length sum).
    pub metric: Option<f64>,
}

/// Decision ledger that accumulates layout decisions across the pipeline.
///
/// Threaded through `compute_layout` so each phase can record its choices.
/// The full ledger is available in `RenderDetailedResult` and can be dumped
/// as JSON via the `--decisions` CLI flag.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DecisionLedger {
    /// Unique trace identifier for this pipeline run.
    pub trace_id: TraceId,
    /// Decisions recorded in order of execution.
    pub entries: Vec<DecisionEntry>,
}

impl DecisionLedger {
    /// Record a decision entry.
    pub fn record(&mut self, phase: &str, what: &str, rationale: &str, metric: Option<f64>) {
        self.entries.push(DecisionEntry {
            phase: phase.to_string(),
            what: what.to_string(),
            rationale: rationale.to_string(),
            metric,
        });
    }

    /// Convenience: record a numeric metric without extensive rationale.
    pub fn metric(&mut self, phase: &str, name: &str, value: f64) {
        self.record(phase, name, "", Some(value));
    }
}

#[derive(Debug, Clone)]
pub struct TextBlock {
    pub lines: Vec<String>,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub struct NodeLayout {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub label: TextBlock,
    pub shape: crate::ir::NodeShape,
    pub style: crate::ir::NodeStyle,
    pub link: Option<crate::ir::NodeLink>,
    pub anchor_subgraph: Option<usize>,
    pub hidden: bool,
    pub icon: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EdgeLayout {
    pub from: String,
    pub to: String,
    pub label: Option<TextBlock>,
    pub start_label: Option<TextBlock>,
    pub end_label: Option<TextBlock>,
    pub label_anchor: Option<(f32, f32)>,
    pub start_label_anchor: Option<(f32, f32)>,
    pub end_label_anchor: Option<(f32, f32)>,
    pub points: Vec<(f32, f32)>,
    pub directed: bool,
    pub arrow_start: bool,
    pub arrow_end: bool,
    pub arrow_start_kind: Option<crate::ir::EdgeArrowhead>,
    pub arrow_end_kind: Option<crate::ir::EdgeArrowhead>,
    pub start_decoration: Option<crate::ir::EdgeDecoration>,
    pub end_decoration: Option<crate::ir::EdgeDecoration>,
    pub style: crate::ir::EdgeStyle,
    pub override_style: crate::ir::EdgeStyleOverride,
}

#[derive(Debug, Clone)]
pub struct SubgraphLayout {
    pub label: String,
    pub label_block: TextBlock,
    pub nodes: Vec<String>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub style: crate::ir::NodeStyle,
    pub icon: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Lifeline {
    pub id: String,
    pub x: f32,
    pub y1: f32,
    pub y2: f32,
}

#[derive(Debug, Clone)]
pub struct SequenceLabel {
    pub x: f32,
    pub y: f32,
    pub text: TextBlock,
}

#[derive(Debug, Clone)]
pub struct SequenceFrameLayout {
    pub kind: crate::ir::SequenceFrameKind,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub label_box: (f32, f32, f32, f32),
    pub label: SequenceLabel,
    pub section_labels: Vec<SequenceLabel>,
    pub dividers: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct SequenceBoxLayout {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub label: Option<TextBlock>,
    pub color: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SequenceNoteLayout {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub label: TextBlock,
    pub position: crate::ir::SequenceNotePosition,
    pub participants: Vec<String>,
    pub index: usize,
}

#[derive(Debug, Clone)]
pub struct StateNoteLayout {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub label: TextBlock,
    pub position: crate::ir::StateNotePosition,
    pub target: String,
}

#[derive(Debug, Clone)]
pub struct SequenceActivationLayout {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub participant: String,
    pub depth: usize,
}

#[derive(Debug, Clone)]
pub struct SequenceNumberLayout {
    pub x: f32,
    pub y: f32,
    pub value: usize,
}

#[derive(Debug, Clone)]
pub struct PieSliceLayout {
    pub label: TextBlock,
    pub value: f32,
    pub start_angle: f32,
    pub end_angle: f32,
    pub color: String,
}

#[derive(Debug, Clone)]
pub struct PieLegendItem {
    pub x: f32,
    pub y: f32,
    pub label: TextBlock,
    pub color: String,
    pub marker_size: f32,
    pub value: f32,
}

#[derive(Debug, Clone)]
pub struct PieTitleLayout {
    pub x: f32,
    pub y: f32,
    pub text: TextBlock,
}

#[derive(Debug, Clone)]
pub struct SankeyNodeLayout {
    pub id: String,
    pub label: String,
    pub total: f32,
    pub rank: usize,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: String,
}

#[derive(Debug, Clone)]
pub struct SankeyLinkLayout {
    pub source: String,
    pub target: String,
    pub value: f32,
    pub thickness: f32,
    pub start: (f32, f32),
    pub end: (f32, f32),
    pub color_start: String,
    pub color_end: String,
    pub gradient_id: String,
}

#[derive(Debug, Clone)]
pub struct SankeyLayout {
    pub width: f32,
    pub height: f32,
    pub node_width: f32,
    pub nodes: Vec<SankeyNodeLayout>,
    pub links: Vec<SankeyLinkLayout>,
}

#[derive(Debug, Clone)]
pub struct GitGraphBranchLabelLayout {
    pub bg_x: f32,
    pub bg_y: f32,
    pub bg_width: f32,
    pub bg_height: f32,
    pub text_x: f32,
    pub text_y: f32,
    pub text_width: f32,
    pub text_height: f32,
}

#[derive(Debug, Clone)]
pub struct GitGraphBranchLayout {
    pub name: String,
    pub index: usize,
    pub pos: f32,
    pub label: GitGraphBranchLabelLayout,
}

#[derive(Debug, Clone)]
pub struct GitGraphTransform {
    pub translate_x: f32,
    pub translate_y: f32,
    pub rotate_deg: f32,
    pub rotate_cx: f32,
    pub rotate_cy: f32,
}

#[derive(Debug, Clone)]
pub struct GitGraphCommitLabelLayout {
    pub text: String,
    pub text_x: f32,
    pub text_y: f32,
    pub bg_x: f32,
    pub bg_y: f32,
    pub bg_width: f32,
    pub bg_height: f32,
    pub transform: Option<GitGraphTransform>,
}

#[derive(Debug, Clone)]
pub struct GitGraphTagLayout {
    pub text: String,
    pub text_x: f32,
    pub text_y: f32,
    pub points: Vec<(f32, f32)>,
    pub hole_x: f32,
    pub hole_y: f32,
    pub transform: Option<GitGraphTransform>,
}

#[derive(Debug, Clone)]
pub struct GitGraphCommitLayout {
    pub id: String,
    pub seq: usize,
    pub branch_index: usize,
    pub x: f32,
    pub y: f32,
    pub axis_pos: f32,
    pub commit_type: crate::ir::GitGraphCommitType,
    pub custom_type: Option<crate::ir::GitGraphCommitType>,
    pub tags: Vec<GitGraphTagLayout>,
    pub label: Option<GitGraphCommitLabelLayout>,
}

#[derive(Debug, Clone)]
pub struct GitGraphArrowLayout {
    pub path: String,
    pub color_index: usize,
}

#[derive(Debug, Clone)]
pub struct GitGraphLayout {
    pub branches: Vec<GitGraphBranchLayout>,
    pub commits: Vec<GitGraphCommitLayout>,
    pub arrows: Vec<GitGraphArrowLayout>,
    pub width: f32,
    pub height: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub max_pos: f32,
    pub direction: Direction,
}

#[derive(Debug, Clone)]
pub struct ErrorLayout {
    pub viewbox_width: f32,
    pub viewbox_height: f32,
    pub render_width: f32,
    pub render_height: f32,
    pub message: String,
    pub version: String,
    pub text_x: f32,
    pub text_y: f32,
    pub text_size: f32,
    pub version_x: f32,
    pub version_y: f32,
    pub version_size: f32,
    pub icon_scale: f32,
    pub icon_tx: f32,
    pub icon_ty: f32,
}

#[derive(Debug, Clone)]
pub struct XYChartBarLayout {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub value: f32,
    pub color: String,
}

#[derive(Debug, Clone)]
pub struct XYChartLineLayout {
    pub points: Vec<(f32, f32)>,
    pub color: String,
}

#[derive(Debug, Clone)]
pub struct XYChartLayout {
    pub title: Option<TextBlock>,
    pub title_y: f32,
    pub x_axis_label: Option<TextBlock>,
    pub x_axis_label_y: f32,
    pub y_axis_label: Option<TextBlock>,
    pub y_axis_label_x: f32,
    pub x_axis_categories: Vec<(String, f32)>,
    pub y_axis_ticks: Vec<(String, f32)>,
    pub bars: Vec<XYChartBarLayout>,
    pub lines: Vec<XYChartLineLayout>,
    pub plot_x: f32,
    pub plot_y: f32,
    pub plot_width: f32,
    pub plot_height: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub struct TimelineEventLayout {
    pub time: TextBlock,
    pub events: Vec<TextBlock>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub circle_y: f32,
}

#[derive(Debug, Clone)]
pub struct TimelineSectionLayout {
    pub label: TextBlock,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub struct TimelineLayout {
    pub title: Option<TextBlock>,
    pub title_y: f32,
    pub events: Vec<TimelineEventLayout>,
    pub sections: Vec<TimelineSectionLayout>,
    pub direction: crate::ir::Direction,
    pub line_y: f32,
    pub line_start_x: f32,
    pub line_end_x: f32,
    pub line_start_y: f32,
    pub line_end_y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub struct JourneyActorLayout {
    pub name: String,
    pub color: String,
    pub x: f32,
    pub y: f32,
    pub radius: f32,
}

#[derive(Debug, Clone)]
pub struct JourneyTaskLayout {
    pub id: String,
    pub label: TextBlock,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub score: Option<f32>,
    pub score_color: String,
    pub score_y: f32,
    pub actors: Vec<String>,
    pub actor_y: Option<f32>,
    pub section_idx: usize,
}

#[derive(Debug, Clone)]
pub struct JourneySectionLayout {
    pub label: TextBlock,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: String,
}

#[derive(Debug, Clone)]
pub struct JourneyLayout {
    pub title: Option<TextBlock>,
    pub title_y: f32,
    pub actors: Vec<JourneyActorLayout>,
    pub actor_label_y: f32,
    pub tasks: Vec<JourneyTaskLayout>,
    pub sections: Vec<JourneySectionLayout>,
    pub baseline: Option<(f32, f32, f32)>,
    pub score_radius: f32,
    pub actor_radius: f32,
    pub actor_gap: f32,
    pub card_gap_y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub struct SequenceData {
    pub lifelines: Vec<Lifeline>,
    pub footboxes: Vec<NodeLayout>,
    pub boxes: Vec<SequenceBoxLayout>,
    pub frames: Vec<SequenceFrameLayout>,
    pub notes: Vec<SequenceNoteLayout>,
    pub activations: Vec<SequenceActivationLayout>,
    pub numbers: Vec<SequenceNumberLayout>,
}

#[derive(Debug, Clone)]
pub struct PieData {
    pub slices: Vec<PieSliceLayout>,
    pub legend: Vec<PieLegendItem>,
    pub center: (f32, f32),
    pub radius: f32,
    pub title: Option<PieTitleLayout>,
}

#[derive(Debug, Clone)]
pub enum DiagramData {
    Graph { state_notes: Vec<StateNoteLayout> },
    Sequence(SequenceData),
    Pie(PieData),
    Quadrant(QuadrantLayout),
    Gantt(GanttLayout),
    Sankey(SankeyLayout),
    GitGraph(GitGraphLayout),
    C4(C4Layout),
    XYChart(XYChartLayout),
    Timeline(TimelineLayout),
    Journey(JourneyLayout),
    Error(ErrorLayout),
}

#[derive(Debug, Clone)]
pub struct Layout {
    pub kind: crate::ir::DiagramKind,
    pub nodes: BTreeMap<String, NodeLayout>,
    pub edges: Vec<EdgeLayout>,
    pub subgraphs: Vec<SubgraphLayout>,
    pub width: f32,
    pub height: f32,
    pub diagram: DiagramData,
}

#[derive(Debug, Clone)]
pub struct C4Layout {
    pub shapes: Vec<C4ShapeLayout>,
    pub boundaries: Vec<C4BoundaryLayout>,
    pub rels: Vec<C4RelLayout>,
    pub viewbox_x: f32,
    pub viewbox_y: f32,
    pub viewbox_width: f32,
    pub viewbox_height: f32,
    pub use_max_width: bool,
}

#[derive(Debug, Clone)]
pub struct C4TextLayout {
    pub text: String,
    pub lines: Vec<String>,
    pub width: f32,
    pub height: f32,
    pub y: f32,
}

#[derive(Debug, Clone)]
pub struct C4ShapeLayout {
    pub id: String,
    pub kind: crate::ir::C4ShapeKind,
    pub bg_color: Option<String>,
    pub border_color: Option<String>,
    pub font_color: Option<String>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub margin: f32,
    pub type_label: C4TextLayout,
    pub label: C4TextLayout,
    pub type_or_techn: Option<C4TextLayout>,
    pub descr: Option<C4TextLayout>,
    pub image_y: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct C4BoundaryLayout {
    pub id: String,
    pub label: C4TextLayout,
    pub boundary_type: Option<C4TextLayout>,
    pub descr: Option<C4TextLayout>,
    pub bg_color: Option<String>,
    pub border_color: Option<String>,
    pub font_color: Option<String>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub struct C4RelLayout {
    pub kind: crate::ir::C4RelKind,
    pub from: String,
    pub to: String,
    pub label: C4TextLayout,
    pub techn: Option<C4TextLayout>,
    pub start: (f32, f32),
    pub end: (f32, f32),
    /// Full routed polyline including `start` and `end`. When the connector
    /// must detour around an intervening shape this has interior waypoints;
    /// for a clear straight connector it is just `[start, end]`.
    pub waypoints: Vec<(f32, f32)>,
    pub offset_x: f32,
    pub offset_y: f32,
    pub line_color: Option<String>,
    pub text_color: Option<String>,
}

#[derive(Debug, Clone)]
pub struct QuadrantLayout {
    pub title: Option<TextBlock>,
    pub title_y: f32,
    pub x_axis_left: Option<TextBlock>,
    pub x_axis_right: Option<TextBlock>,
    pub y_axis_bottom: Option<TextBlock>,
    pub y_axis_top: Option<TextBlock>,
    pub quadrant_labels: [Option<TextBlock>; 4],
    pub points: Vec<QuadrantPointLayout>,
    pub grid_x: f32,
    pub grid_y: f32,
    pub grid_width: f32,
    pub grid_height: f32,
}

#[derive(Debug, Clone)]
pub struct QuadrantPointLayout {
    pub label: TextBlock,
    pub x: f32,
    pub y: f32,
    pub color: String,
}

#[derive(Debug, Clone)]
pub struct GanttLayout {
    pub title: Option<TextBlock>,
    pub sections: Vec<GanttSectionLayout>,
    pub tasks: Vec<GanttTaskLayout>,
    pub time_start: f32,
    pub time_end: f32,
    pub chart_x: f32,
    pub chart_y: f32,
    pub chart_width: f32,
    pub chart_height: f32,
    pub row_height: f32,
    pub label_x: f32,
    pub label_width: f32,
    pub section_label_x: f32,
    pub section_label_width: f32,
    pub task_label_x: f32,
    pub task_label_width: f32,
    pub title_y: f32,
    pub ticks: Vec<GanttTick>,
    pub compact: bool,
}

#[derive(Debug, Clone)]
pub struct GanttSectionLayout {
    pub label: TextBlock,
    pub y: f32,
    pub height: f32,
    pub color: String,
    pub band_color: String,
}

#[derive(Debug, Clone)]
pub struct GanttTaskLayout {
    pub label: TextBlock,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: String,
    pub start: f32,
    pub duration: f32,
    pub status: Option<crate::ir::GanttStatus>,
}

#[derive(Debug, Clone)]
pub struct GanttTick {
    pub x: f32,
    pub label: String,
}

/// Layout algorithm strategy, auto-selected by diagram type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LayoutAlgorithm {
    /// Sugiyama layered layout (flowcharts, class, state, ER)
    #[default]
    Sugiyama,
    /// Force-directed placement (simple graphs, mindmaps)
    ForceDirected,
    /// Tree layout (mindmap, timeline)
    Tree,
    /// Radial layout around a center
    Radial,
    /// Sequence diagram layout (top-to-bottom lifelines)
    Sequence,
    /// Pie chart layout (circular)
    Pie,
    /// Sankey diagram layout (flow magnitude)
    Sankey,
    /// Grid layout (block, kanban)
    Grid,
    /// Gantt chart timeline layout
    Gantt,
    /// Git graph layout (branch topology)
    GitGraph,
    /// Quadrant chart layout
    Quadrant,
    /// XY chart layout (cartesian)
    XYChart,
    /// Journey map layout
    Journey,
    /// C4 architecture layout
    C4,
    /// Error fallback layout
    Error,
}

impl LayoutAlgorithm {
    pub fn auto_select(kind: &crate::ir::DiagramKind) -> Self {
        match kind {
            crate::ir::DiagramKind::Flowchart
            | crate::ir::DiagramKind::Class
            | crate::ir::DiagramKind::State
            | crate::ir::DiagramKind::Er
            | crate::ir::DiagramKind::Requirement
            | crate::ir::DiagramKind::Packet => Self::Sugiyama,
            crate::ir::DiagramKind::Sequence | crate::ir::DiagramKind::ZenUML => Self::Sequence,
            crate::ir::DiagramKind::Pie => Self::Pie,
            crate::ir::DiagramKind::Mindmap => Self::Tree,
            crate::ir::DiagramKind::Journey => Self::Journey,
            crate::ir::DiagramKind::Timeline => Self::Tree,
            crate::ir::DiagramKind::Gantt => Self::Gantt,
            crate::ir::DiagramKind::GitGraph => Self::GitGraph,
            crate::ir::DiagramKind::C4 => Self::C4,
            crate::ir::DiagramKind::Sankey => Self::Sankey,
            crate::ir::DiagramKind::Quadrant => Self::Quadrant,
            crate::ir::DiagramKind::Block => Self::Grid,
            crate::ir::DiagramKind::Kanban => Self::Grid,
            crate::ir::DiagramKind::Architecture => Self::Grid,
            crate::ir::DiagramKind::Radar => Self::Pie,
            crate::ir::DiagramKind::Treemap => Self::Grid,
            crate::ir::DiagramKind::XYChart => Self::XYChart,
        }
    }

    /// Parse a CLI/config algorithm name (`auto` returns `None`).
    pub fn parse_name(raw: &str) -> Result<Option<Self>, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(None),
            "sugiyama" => Ok(Some(Self::Sugiyama)),
            "force" | "force-directed" | "forcedirected" => Ok(Some(Self::ForceDirected)),
            "tree" => Ok(Some(Self::Tree)),
            "radial" => Ok(Some(Self::Radial)),
            "sequence" => Ok(Some(Self::Sequence)),
            "pie" => Ok(Some(Self::Pie)),
            "sankey" => Ok(Some(Self::Sankey)),
            "grid" => Ok(Some(Self::Grid)),
            "gantt" => Ok(Some(Self::Gantt)),
            "gitgraph" | "git-graph" => Ok(Some(Self::GitGraph)),
            "quadrant" => Ok(Some(Self::Quadrant)),
            "xychart" | "xy-chart" => Ok(Some(Self::XYChart)),
            "journey" => Ok(Some(Self::Journey)),
            "c4" => Ok(Some(Self::C4)),
            "error" => Ok(Some(Self::Error)),
            other => Err(format!("unknown layout algorithm '{other}'")),
        }
    }
}

impl std::fmt::Display for LayoutAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Sugiyama => "sugiyama",
            Self::ForceDirected => "force-directed",
            Self::Tree => "tree",
            Self::Radial => "radial",
            Self::Sequence => "sequence",
            Self::Pie => "pie",
            Self::Sankey => "sankey",
            Self::Grid => "grid",
            Self::Gantt => "gantt",
            Self::GitGraph => "git-graph",
            Self::Quadrant => "quadrant",
            Self::XYChart => "xychart",
            Self::Journey => "journey",
            Self::C4 => "c4",
            Self::Error => "error",
        };
        f.write_str(name)
    }
}

/// Cycle-breaking strategy for directed graphs with cycles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CycleStrategy {
    /// Greedy feedback arc set (fast, reasonable quality)
    #[default]
    Greedy,
    /// DFS back-edge detection
    DfsBackEdge,
    /// Minimum feedback arc set approximation
    Mfas,
    /// Full SCC-aware cycle detection with cluster collapse
    CycleAwareScc,
}

impl CycleStrategy {
    /// Parse a CLI/config cycle-strategy name.
    pub fn parse_name(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "greedy" => Ok(Self::Greedy),
            "dfs" | "dfs-back-edge" | "dfsbackedge" => Ok(Self::DfsBackEdge),
            "mfas" => Ok(Self::Mfas),
            "scc" | "cycle-aware-scc" | "cycleawarescc" => Ok(Self::CycleAwareScc),
            other => Err(format!("unknown cycle strategy '{other}'")),
        }
    }
}

impl std::fmt::Display for CycleStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Greedy => "greedy",
            Self::DfsBackEdge => "dfs-back-edge",
            Self::Mfas => "mfas",
            Self::CycleAwareScc => "cycle-aware-scc",
        };
        f.write_str(name)
    }
}

impl RenderScene {
    /// Build a RenderScene from a completed Layout.
    pub fn from_layout(
        layout: &Layout,
        theme: &crate::theme::Theme,
        _config: &crate::config::LayoutConfig,
    ) -> Self {
        let mut scene = RenderScene {
            width: layout.width,
            height: layout.height,
            groups: Vec::new(),
        };

        // Subgraph backgrounds (drawn first)
        for sg in &layout.subgraphs {
            let mut group = RenderGroup {
                label: Some(sg.label.clone()),
                items: Vec::new(),
            };
            group.items.push(RenderItem::Rect {
                x: sg.x,
                y: sg.y,
                w: sg.width,
                h: sg.height,
                rx: Some(8.0),
                fill: Some(
                    sg.style
                        .fill
                        .clone()
                        .unwrap_or_else(|| "#ffffff".to_string()),
                ),
                stroke: Some(
                    sg.style
                        .stroke
                        .clone()
                        .unwrap_or_else(|| "#cccccc".to_string()),
                ),
                stroke_width: sg.style.stroke_width.unwrap_or(1.0),
            });
            // Subgraph label
            group.items.push(RenderItem::Text {
                x: sg.x + 10.0,
                y: sg.y + sg.label_block.height + 4.0,
                text: sg.label_block.lines.join(" "),
                font_size: 14.0,
                font_family: None,
                fill: Some(theme.text_color.clone()),
                anchor: TextAnchor::Start,
            });
            scene.groups.push(group);
        }

        // Nodes
        let mut node_group = RenderGroup {
            label: Some("nodes".to_string()),
            items: Vec::new(),
        };
        for node in layout.nodes.values() {
            if node.hidden {
                continue;
            }
            let rx = match node.shape {
                crate::ir::NodeShape::RoundRect | crate::ir::NodeShape::Stadium => Some(6.0),
                crate::ir::NodeShape::Diamond => None,
                _ => None,
            };
            node_group.items.push(RenderItem::Rect {
                x: node.x,
                y: node.y,
                w: node.width,
                h: node.height,
                rx,
                fill: Some(
                    node.style
                        .fill
                        .clone()
                        .unwrap_or_else(|| theme.primary_color.clone()),
                ),
                stroke: Some(
                    node.style
                        .stroke
                        .clone()
                        .unwrap_or_else(|| theme.primary_border_color.clone()),
                ),
                stroke_width: node.style.stroke_width.unwrap_or(2.0),
            });
            node_group.items.push(RenderItem::Text {
                x: node.x + node.width / 2.0,
                y: node.y + node.height / 2.0 + node.label.height / 4.0,
                text: node.label.lines.join(" "),
                font_size: 14.0,
                font_family: None,
                fill: Some(
                    node.style
                        .text_color
                        .clone()
                        .unwrap_or_else(|| theme.primary_text_color.clone()),
                ),
                anchor: TextAnchor::Middle,
            });
        }
        scene.groups.push(node_group);

        // Edges
        let mut edge_group = RenderGroup {
            label: Some("edges".to_string()),
            items: Vec::new(),
        };
        for edge in &layout.edges {
            if edge.points.len() >= 2 {
                edge_group.items.push(RenderItem::Polyline {
                    points: edge.points.clone(),
                    stroke: Some(theme.line_color.clone()),
                    stroke_width: 2.0,
                    fill: None,
                });
                // Edge label
                if let Some(label) = &edge.label {
                    if let Some(anchor) = edge.label_anchor {
                        edge_group.items.push(RenderItem::Text {
                            x: anchor.0,
                            y: anchor.1 + label.height / 4.0,
                            text: label.lines.join(" "),
                            font_size: 12.0,
                            font_family: None,
                            fill: Some(theme.text_color.clone()),
                            anchor: TextAnchor::Middle,
                        });
                    }
                }
            }
        }
        scene.groups.push(edge_group);

        scene
    }
}

#[cfg(test)]
mod pattern_tests {
    use super::*;
    use crate::ir::DiagramKind;

    #[test]
    fn layout_algorithm_auto_selects_flowchart_sugiyama() {
        assert_eq!(
            LayoutAlgorithm::auto_select(&DiagramKind::Flowchart),
            LayoutAlgorithm::Sugiyama
        );
        assert_eq!(
            LayoutAlgorithm::auto_select(&DiagramKind::Sequence),
            LayoutAlgorithm::Sequence
        );
    }

    #[test]
    fn layout_algorithm_parse_name_accepts_auto_and_aliases() {
        assert_eq!(LayoutAlgorithm::parse_name("auto").unwrap(), None);
        assert_eq!(
            LayoutAlgorithm::parse_name("sugiyama").unwrap(),
            Some(LayoutAlgorithm::Sugiyama)
        );
        assert_eq!(
            LayoutAlgorithm::parse_name("force-directed").unwrap(),
            Some(LayoutAlgorithm::ForceDirected)
        );
        assert!(LayoutAlgorithm::parse_name("nope").is_err());
    }

    #[test]
    fn cycle_strategy_display_and_parse_roundtrip() {
        for strategy in [
            CycleStrategy::Greedy,
            CycleStrategy::DfsBackEdge,
            CycleStrategy::Mfas,
            CycleStrategy::CycleAwareScc,
        ] {
            let parsed = CycleStrategy::parse_name(&strategy.to_string()).unwrap();
            assert_eq!(parsed, strategy);
        }
    }

    #[test]
    fn decision_ledger_records_entries_with_trace_id() {
        let mut ledger = DecisionLedger::default();
        ledger.record("dispatch", "diagram_type=flowchart", "test", None);
        ledger.metric("finalize", "nodes", 3.0);
        assert!(!ledger.trace_id.to_string().is_empty());
        assert_eq!(ledger.entries.len(), 2);
        assert_eq!(ledger.entries[1].metric, Some(3.0));
    }
}
