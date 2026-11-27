//! PNG rendering for a graph traversal result.
//!
//! The layout is deliberately simple: one column per hop, a highlighted start
//! node, boxes for terms, and coloured edges. It is a diagnostic view, not a
//! force-directed layout. Text uses the built-in 8x8 bitmap font so the server
//! needs no font asset and no external renderer.

use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use font8x8::{BASIC_FONTS, UnicodeFonts};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};

use crate::dictionary::{EdgeDirection, GraphResponse};

const BOX_WIDTH: u32 = 340;
const BOX_HEIGHT: u32 = 64;
const COLUMN_WIDTH: u32 = 430;
const MARGIN: u32 = 40;
const HEADER_HEIGHT: u32 = 90;
const OUTGOING: Rgb<u8> = Rgb([0, 140, 0]);
const INCOMING: Rgb<u8> = Rgb([0, 80, 200]);
const BORDER: Rgb<u8> = Rgb([70, 70, 70]);
const START_FILL: Rgb<u8> = Rgb([255, 245, 190]);
const START_BORDER: Rgb<u8> = Rgb([180, 120, 0]);

/// A rendered PNG held in memory and written to disk.
#[derive(Debug)]
pub(crate) struct RenderedGraph {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy)]
struct NodeBox {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl NodeBox {
    fn center(self) -> (i32, i32) {
        (
            (self.x + self.width / 2) as i32,
            (self.y + self.height / 2) as i32,
        )
    }
}

/// Render `graph` to `<directory>/graph-<fingerprint>.png`.
pub(crate) fn render(graph: &GraphResponse, directory: &Path) -> Result<RenderedGraph, String> {
    let max_distance = graph
        .nodes
        .iter()
        .map(|node| node.distance)
        .max()
        .unwrap_or(0);
    let columns = max_distance + 1;
    let width = (80 + columns as u32 * COLUMN_WIDTH).max(1000);
    let mut groups: Vec<Vec<&crate::dictionary::GraphNode>> = vec![Vec::new(); columns];
    for node in &graph.nodes {
        let index = node.distance.min(columns.saturating_sub(1));
        groups[index].push(node);
    }
    let max_group = groups.iter().map(Vec::len).max().unwrap_or(1).max(1);
    let height = (HEADER_HEIGHT + 80 + (max_group as u32) * 80).clamp(800, 5000);

    let mut image = RgbImage::from_pixel(width, height, Rgb([250, 250, 248]));
    draw_header(&mut image, graph, width);

    let mut boxes: HashMap<&str, NodeBox> = HashMap::new();
    let start_box = NodeBox {
        x: MARGIN,
        y: height / 2 - BOX_HEIGHT / 2,
        width: BOX_WIDTH,
        height: BOX_HEIGHT,
    };
    boxes.insert(graph.term.as_str(), start_box);

    for (distance, group) in groups.iter().enumerate().skip(1) {
        let count = group.len() as u32;
        if count == 0 {
            continue;
        }
        let available = height.saturating_sub(HEADER_HEIGHT + 60);
        let spacing = available / (count + 1);
        let column_x = MARGIN + distance as u32 * COLUMN_WIDTH;
        for (index, node) in group.iter().enumerate() {
            let center_y = HEADER_HEIGHT + 30 + spacing * (index as u32 + 1);
            let node_box = NodeBox {
                x: column_x,
                y: center_y.saturating_sub(BOX_HEIGHT / 2),
                width: BOX_WIDTH,
                height: BOX_HEIGHT,
            };
            boxes.entry(node.term.as_str()).or_insert(node_box);
        }
    }

    // Edges first, so boxes cover their endpoints.
    for node in &graph.nodes {
        let (source, target, color) = match node.edge_direction {
            EdgeDirection::Outgoing => (node.via.as_str(), node.term.as_str(), OUTGOING),
            EdgeDirection::Incoming => (node.term.as_str(), node.via.as_str(), INCOMING),
        };
        let (Some(source), Some(target)) = (boxes.get(source), boxes.get(target)) else {
            continue;
        };
        draw_edge(&mut image, *source, *target, color);
    }

    draw_node_box(
        &mut image,
        start_box,
        &graph.term,
        graph.anchor.as_deref(),
        "start",
        true,
        BORDER,
    );
    for node in &graph.nodes {
        let Some(node_box) = boxes.get(node.term.as_str()) else {
            continue;
        };
        let detail = node
            .anchor
            .as_deref()
            .or(node.id.as_deref())
            .unwrap_or(node.file.as_str());
        let meta = format!("d{} | {}", node.distance, node.edge_direction.as_str());
        let color = match node.edge_direction {
            EdgeDirection::Outgoing => OUTGOING,
            EdgeDirection::Incoming => INCOMING,
        };
        draw_node_box(
            &mut image,
            *node_box,
            &node.term,
            Some(detail),
            &meta,
            false,
            color,
        );
    }

    if graph.nodes.is_empty() {
        draw_text(
            &mut image,
            MARGIN,
            HEADER_HEIGHT + 30,
            "No related entries in this direction and depth.",
            2,
            Rgb([90, 90, 90]),
        );
    }

    let mut bytes = Vec::new();
    DynamicImage::ImageRgb8(image)
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .map_err(|error| format!("Cannot encode the graph image: {error}"))?;

    fs::create_dir_all(directory)
        .map_err(|error| format!("Cannot create {}: {error}", directory.display()))?;
    let path = directory.join(format!("graph-{:016x}.png", fingerprint(graph)));
    fs::write(&path, &bytes)
        .map_err(|error| format!("Cannot write {}: {error}", path.display()))?;

    Ok(RenderedGraph {
        path,
        bytes,
        width,
        height,
    })
}

fn draw_header(image: &mut RgbImage, graph: &GraphResponse, width: u32) {
    let title = format!(
        "{} | {} | depth {} | {} reachable",
        graph.term,
        graph.direction.as_str(),
        graph.depth,
        graph.total
    );
    draw_text(image, MARGIN, 18, &title, 2, Rgb([25, 25, 25]));
    draw_text(
        image,
        MARGIN,
        48,
        "green = outgoing | blue = incoming | gold = start",
        1,
        Rgb([90, 90, 90]),
    );
    draw_text(
        image,
        MARGIN,
        64,
        &format!("showing {} of {} nodes", graph.nodes.len(), graph.total),
        1,
        Rgb([90, 90, 90]),
    );
    draw_line(
        image,
        MARGIN as i32,
        (HEADER_HEIGHT - 8) as i32,
        (width - MARGIN) as i32,
        (HEADER_HEIGHT - 8) as i32,
        Rgb([200, 200, 200]),
    );
}

fn draw_node_box(
    image: &mut RgbImage,
    node: NodeBox,
    term: &str,
    anchor: Option<&str>,
    meta: &str,
    start: bool,
    border: Rgb<u8>,
) {
    fill_rect(
        image,
        node.x,
        node.y,
        node.width,
        node.height,
        if start {
            START_FILL
        } else {
            Rgb([255, 255, 255])
        },
    );
    draw_rect(
        image,
        node.x,
        node.y,
        node.width,
        node.height,
        if start { START_BORDER } else { border },
    );
    let inner = node.width.saturating_sub(16);
    draw_text(
        image,
        node.x + 8,
        node.y + 7,
        &truncate_text(term, (inner / 16) as usize),
        2,
        Rgb([20, 20, 20]),
    );
    if let Some(anchor) = anchor {
        draw_text(
            image,
            node.x + 8,
            node.y + 30,
            &truncate_text(anchor, (inner / 8) as usize),
            1,
            Rgb([80, 80, 80]),
        );
    }
    draw_text(
        image,
        node.x + 8,
        node.y + 46,
        &truncate_text(meta, (inner / 8) as usize),
        1,
        border,
    );
}

fn draw_edge(image: &mut RgbImage, source: NodeBox, target: NodeBox, color: Rgb<u8>) {
    let from = source.center();
    let to = target.center();
    let end = clip_to_box(from, to, target);
    draw_line(image, from.0, from.1, end.0, end.1, color);
    draw_arrow(image, from, end, color);
}

fn clip_to_box(from: (i32, i32), to: (i32, i32), node: NodeBox) -> (i32, i32) {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    if dx == 0 && dy == 0 {
        return to;
    }
    let half_w = node.width as f32 / 2.0;
    let half_h = node.height as f32 / 2.0;
    let scale_x = if dx == 0 {
        f32::INFINITY
    } else {
        half_w / dx.abs() as f32
    };
    let scale_y = if dy == 0 {
        f32::INFINITY
    } else {
        half_h / dy.abs() as f32
    };
    let scale = scale_x.min(scale_y);
    (
        to.0 - (dx as f32 * scale) as i32,
        to.1 - (dy as f32 * scale) as i32,
    )
}

fn draw_arrow(image: &mut RgbImage, from: (i32, i32), tip: (i32, i32), color: Rgb<u8>) {
    let dx = tip.0 - from.0;
    let dy = tip.1 - from.1;
    let length = ((dx * dx + dy * dy) as f32).sqrt();
    if length < 1.0 {
        return;
    }
    let ux = dx as f32 / length;
    let uy = dy as f32 / length;
    let size = 10.0;
    let left = (
        tip.0 - (ux * size - uy * size * 0.55) as i32,
        tip.1 - (uy * size + ux * size * 0.55) as i32,
    );
    let right = (
        tip.0 - (ux * size + uy * size * 0.55) as i32,
        tip.1 - (uy * size - ux * size * 0.55) as i32,
    );
    draw_line(image, tip.0, tip.1, left.0, left.1, color);
    draw_line(image, tip.0, tip.1, right.0, right.1, color);
}

fn draw_text(image: &mut RgbImage, x: u32, y: u32, text: &str, scale: u32, color: Rgb<u8>) {
    let scale = scale.max(1);
    let mut cursor = x as i32;
    for character in text.chars() {
        let glyph = BASIC_FONTS.get(character).or_else(|| BASIC_FONTS.get('?'));
        let Some(glyph) = glyph else {
            continue;
        };
        for (row, bits) in glyph.iter().enumerate() {
            for column in 0..8 {
                if bits & (1 << column) != 0 {
                    let pixel_x = (cursor + column * scale as i32).max(0) as u32;
                    let pixel_y = y + row as u32 * scale;
                    fill_rect(image, pixel_x, pixel_y, scale, scale, color);
                }
            }
        }
        cursor += 8 * scale as i32;
    }
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    let max_chars = max_chars.max(3);
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_owned();
    }
    let mut truncated: String = text.chars().take(max_chars - 2).collect();
    truncated.push_str("..");
    truncated
}

fn fill_rect(image: &mut RgbImage, x: u32, y: u32, width: u32, height: u32, color: Rgb<u8>) {
    let max_x = x.saturating_add(width).min(image.width());
    let max_y = y.saturating_add(height).min(image.height());
    for py in y..max_y {
        for px in x..max_x {
            image.put_pixel(px, py, color);
        }
    }
}

fn draw_rect(image: &mut RgbImage, x: u32, y: u32, width: u32, height: u32, color: Rgb<u8>) {
    draw_line(
        image,
        x as i32,
        y as i32,
        (x + width) as i32,
        y as i32,
        color,
    );
    draw_line(
        image,
        (x + width) as i32,
        y as i32,
        (x + width) as i32,
        (y + height) as i32,
        color,
    );
    draw_line(
        image,
        (x + width) as i32,
        (y + height) as i32,
        x as i32,
        (y + height) as i32,
        color,
    );
    draw_line(
        image,
        x as i32,
        (y + height) as i32,
        x as i32,
        y as i32,
        color,
    );
}

fn draw_line(image: &mut RgbImage, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgb<u8>) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut error = dx + dy;
    let (mut x, mut y) = (x0, y0);
    loop {
        for offset_x in -1..=1 {
            for offset_y in -1..=1 {
                let px = x + offset_x;
                let py = y + offset_y;
                if px >= 0 && py >= 0 && (px as u32) < image.width() && (py as u32) < image.height()
                {
                    image.put_pixel(px as u32, py as u32, color);
                }
            }
        }
        if x == x1 && y == y1 {
            break;
        }
        let doubled = 2 * error;
        if doubled >= dy {
            error += dy;
            x += sx;
        }
        if doubled <= dx {
            error += dx;
            y += sy;
        }
    }
}

fn fingerprint(graph: &GraphResponse) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    feed(&mut hash, &graph.query);
    feed(&mut hash, graph.direction.as_str());
    feed(&mut hash, &graph.depth.to_string());
    for node in &graph.nodes {
        feed(&mut hash, &node.term);
        feed(&mut hash, &node.distance.to_string());
        feed(&mut hash, node.edge_direction.as_str());
    }
    hash
}

fn feed(hash: &mut u64, text: &str) {
    for byte in text.as_bytes() {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    *hash ^= 0xff;
    *hash = hash.wrapping_mul(0x100_0000_01b3);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::{EdgeDirection, GraphDirection, GraphNode, GraphResponse, Source};

    fn response() -> GraphResponse {
        GraphResponse {
            query: "alpha".to_owned(),
            term: "alpha".to_owned(),
            anchor: Some("a1ALP".to_owned()),
            direction: GraphDirection::Both,
            depth: 1,
            nodes: vec![
                GraphNode {
                    source: Source::Dictionary,
                    term: "greater alpha".to_owned(),
                    id: None,
                    anchor: Some("a1BET".to_owned()),
                    file: "14-g.md".to_owned(),
                    citation: "Test Dictionary, 14-g.md#a1BET".to_owned(),
                    source_file: Some("C:\\data\\14-g.md".to_owned()),
                    line: Some(3),
                    distance: 1,
                    edge_direction: EdgeDirection::Outgoing,
                    via: "alpha".to_owned(),
                    link_text: "greater alpha".to_owned(),
                    redirect: false,
                },
                GraphNode {
                    source: Source::Dictionary,
                    term: "alpha alias".to_owned(),
                    id: None,
                    anchor: Some("a1AAL".to_owned()),
                    file: "08-a.md".to_owned(),
                    citation: "Test Dictionary, 08-a.md#a1AAL".to_owned(),
                    source_file: None,
                    line: Some(10),
                    distance: 1,
                    edge_direction: EdgeDirection::Incoming,
                    via: "alpha alias".to_owned(),
                    link_text: "alpha".to_owned(),
                    redirect: false,
                },
            ],
            total: 2,
            truncated: false,
        }
    }

    #[test]
    fn renders_a_png_with_the_expected_dimensions() {
        let directory = std::env::temp_dir().join(format!(
            "mcp-chinese-dict-graph-image-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        let rendered = render(&response(), &directory).expect("the graph renders");
        assert!(rendered.path.exists());
        assert_eq!(&rendered.bytes[..8], b"\x89PNG\r\n\x1a\n");
        let decoded = image::load_from_memory(&rendered.bytes).expect("the PNG decodes");
        assert_eq!(decoded.width(), rendered.width);
        assert_eq!(decoded.height(), rendered.height);
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn empty_graph_still_renders() {
        let directory = std::env::temp_dir().join(format!(
            "mcp-chinese-dict-graph-image-empty-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        let mut graph = response();
        graph.nodes.clear();
        graph.total = 0;
        let rendered = render(&graph, &directory).expect("an empty graph renders");
        assert!(rendered.bytes.starts_with(b"\x89PNG"));
        let _ = fs::remove_dir_all(&directory);
    }
}
