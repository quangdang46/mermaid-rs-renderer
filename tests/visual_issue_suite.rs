use mermaid_rs_renderer::{
    DiagramKind, LayoutConfig, NodeShape, Theme, compute_layout, parse_mermaid, render_svg,
};

fn render(
    input: &str,
) -> (
    mermaid_rs_renderer::Graph,
    mermaid_rs_renderer::Layout,
    String,
) {
    let parsed = parse_mermaid(input).expect("diagram should parse");
    let theme = Theme::modern();
    let config = LayoutConfig::default();
    let layout = compute_layout(&parsed.graph, &theme, &config);
    let svg = render_svg(&layout, &theme, &config);
    (parsed.graph, layout, svg)
}

#[test]
fn architecture_iconify_icons_render_as_symbols_not_broken_question_marks() {
    let input = r#"architecture-beta
    group api(logos:aws-lambda)[API]

    service db(logos:aws-aurora)[Database] in api
    service disk1(logos:aws-glacier)[Storage] in api
    service disk2(logos:aws-s3)[Storage] in api
    service server(logos:aws-ec2)[Server] in api

    db:L -- R:server
    disk1:T -- B:server
    disk2:T -- B:db
"#;

    let (graph, layout, svg) = render(input);
    assert_eq!(graph.kind, DiagramKind::Architecture);
    assert_eq!(graph.nodes.len(), 4);
    assert_eq!(layout.edges.len(), 3);
    assert!(
        !svg.contains(">?</text>") && !svg.contains(">?</tspan>"),
        "registered/Iconify icons should not render as broken question marks"
    );
    assert!(
        svg.contains('λ'),
        "lambda icon should get a symbolic fallback"
    );
    // Port directions must drive grid placement (issue #112): server sits to
    // the left of db (db:L -- R:server), disk1 below server, disk2 below db.
    let node = |id: &str| layout.nodes.get(id).expect(id);
    let (db, server, disk1, disk2) = (node("db"), node("server"), node("disk1"), node("disk2"));
    assert!(
        server.x + server.width <= db.x + 1.0,
        "server should be left of db"
    );
    assert!(
        disk1.y >= server.y + server.height - 1.0,
        "disk1 should be below server"
    );
    assert!(
        disk2.y >= db.y + db.height - 1.0,
        "disk2 should be below db"
    );
}

#[test]
fn architecture_group_edge_modifiers_do_not_create_phantom_nodes() {
    let input = r#"architecture-beta
    group groupOne(cloud)[One]
    group groupTwo(cloud)[Two]
    service server(server)[Server] in groupOne
    service subnet(database)[Subnet] in groupTwo
    server{group}:B --> T:subnet{group}
"#;

    let (graph, layout, svg) = render(input);
    assert!(graph.nodes.contains_key("server"));
    assert!(graph.nodes.contains_key("subnet"));
    assert!(
        graph.nodes.keys().all(|id| !id.contains("{group}")),
        "{{group}} edge modifiers must not become phantom service ids"
    );
    assert_eq!(graph.edges[0].from, "server");
    assert_eq!(graph.edges[0].to, "subnet");
    assert_eq!(layout.nodes.len(), 2);
    assert!(svg.contains("marker-end"));
}

#[test]
fn architecture_junctions_are_compact_routing_points() {
    let input = r#"architecture-beta
    service left_disk(disk)[Disk]
    service top_gateway(internet)[Gateway]
    junction junctionCenter
    junction junctionRight

    left_disk:R -- L:junctionCenter
    junctionCenter:R -- L:junctionRight
    top_gateway:B -- T:junctionRight
"#;

    let (graph, layout, svg) = render(input);
    let center = graph.nodes.get("junctionCenter").expect("junction parsed");
    assert_eq!(center.shape, NodeShape::Circle);
    assert_eq!(center.icon.as_deref(), Some("junction"));

    let center_layout = layout
        .nodes
        .get("junctionCenter")
        .expect("junction laid out");
    assert!(
        center_layout.width <= 24.0 && center_layout.height <= 24.0,
        "junctions should be compact routing dots, got {}x{}",
        center_layout.width,
        center_layout.height
    );
    assert!(svg.contains("<circle"));
    assert!(
        !svg.contains(">junctionCenter<"),
        "junction ids should not render as service labels"
    );
}

#[test]
fn display_math_labels_are_rendered_readably_in_svg_text() {
    let input = r#"graph LR
      A["$$x^2$$"] -->|"$$\sqrt{x+3}$$"| B("$$\frac{1}{2}$$")
      A -->|"$$\overbrace{a+b+c}^{\text{note}}$$"| C("$$\pi r^2$$")
"#;

    let (_graph, _layout, svg) = render(input);
    assert!(svg.contains("x²"));
    assert!(svg.contains("√"));
    assert!(svg.contains("(1)/(2)"));
    assert!(svg.contains("π r²"));
    assert!(
        !svg.contains("$$") && !svg.contains("\\sqrt") && !svg.contains("\\frac"),
        "raw TeX delimiters/commands should not leak into visible SVG text"
    );
}
