use crate::layout::{RenderItem, RenderScene};
use crate::theme::Theme;

/// Render a Layout to terminal text (Unicode box drawing).
pub fn render_term(
    ir: &crate::ir::Graph,
    theme: &Theme,
    config: &crate::config::LayoutConfig,
) -> String {
    let layout = crate::layout::compute_layout(ir, theme, config);
    render_term_layout(&layout, theme)
}

/// Render a Layout to terminal text.
pub fn render_term_layout(layout: &crate::layout::Layout, theme: &Theme) -> String {
    let scene = RenderScene::from_layout(layout, theme, &crate::config::LayoutConfig::default());
    render_term_scene(&scene)
}

/// Render a RenderScene to terminal text with Unicode box drawing.
pub fn render_term_scene(scene: &RenderScene) -> String {
    let cols = (scene.width as usize).clamp(20, 200);
    let rows = (scene.height as usize / 2).clamp(10, 100);

    // Collect text positions from the scene
    let mut text_items: Vec<(f32, f32, &str)> = Vec::new();
    let mut rects: Vec<(f32, f32, f32, f32)> = Vec::new();
    for group in &scene.groups {
        for item in &group.items {
            match item {
                RenderItem::Text { x, y, text, .. } => {
                    text_items.push((*x, *y, text));
                }
                RenderItem::Rect { x, y, w, h, .. } => {
                    rects.push((*x, *y, *w, *h));
                }
                _ => {}
            }
        }
    }

    // Build character grid
    let scale_x = if scene.width > 0.0 {
        cols as f32 / scene.width
    } else {
        1.0
    };
    let scale_y = if scene.height > 0.0 {
        rows as f32 / scene.height
    } else {
        1.0
    };

    // Draw rect outlines
    let mut grid: Vec<Vec<char>> = vec![vec![' '; cols]; rows];
    let h_wall = '─';
    let v_wall = '│';
    let tl = '┌';
    let tr = '┐';
    let bl = '└';
    let br = '┘';

    for (x, y, w, h) in &rects {
        let sx = (*x * scale_x) as usize;
        let sy = (*y * scale_y) as usize;
        let sw = ((*x + *w) * scale_x) as usize;
        let sh = ((*y + *h) * scale_y) as usize;
        let sx = sx.min(cols - 1);
        let sy = sy.min(rows - 1);
        let sw = sw.min(cols - 1);
        let sh = sh.min(rows - 1);

        if sy < rows && sx < cols {
            grid[sy][sx] = tl;
        }
        if sy < rows && sw < cols {
            grid[sy][sw] = tr;
        }
        if sh < rows && sx < cols {
            grid[sh][sx] = bl;
        }
        if sh < rows && sw < cols {
            grid[sh][sw] = br;
        }
        for cx in (sx + 1)..sw {
            if sy < rows {
                grid[sy][cx] = h_wall;
            }
            if sh < rows {
                grid[sh][cx] = h_wall;
            }
        }
        for cy in (sy + 1)..sh {
            grid[cy][sx] = v_wall;
            grid[cy][sw] = v_wall;
        }
    }

    // Place text
    for (x, y, text) in &text_items {
        let tx = (*x * scale_x) as usize;
        let ty = (*y * scale_y) as usize;
        if ty < rows {
            for (ci, ch) in text.chars().enumerate() {
                let cx = tx + ci;
                if cx < cols {
                    grid[ty][cx] = ch;
                }
            }
        }
    }

    // Render grid
    let mut output = String::new();
    for row in &grid {
        let line: String = row.iter().collect();
        output.push_str(&line);
        output.push('\n');
    }

    output
}
