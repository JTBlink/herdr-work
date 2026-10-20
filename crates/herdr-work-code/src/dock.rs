//! Pure docking geometry mechanically moved from Sidebar launch.rs.
//! No plugin identity, configuration or process lifecycle is read here.
use serde::Deserialize;
const MIN_SIDEBAR_SHARE: f64 = 0.15;
const MAX_SIDEBAR_SHARE: f64 = 0.5;
/// Herdr clamps split ratios to at least ten percent.
pub const PRODUCT_MIN_SIDEBAR_SHARE: f64 = 0.1;

#[derive(Deserialize)]
pub(crate) struct LayoutMsg {
    pub(crate) result: LayoutResult,
}

#[derive(Deserialize)]
pub(crate) struct LayoutResult {
    pub(crate) layout: Layout,
}

#[derive(Deserialize)]
pub(crate) struct Layout {
    #[serde(default)]
    pub(crate) panes: Vec<LayoutPane>,
    #[serde(default)]
    pub(crate) splits: Vec<LayoutSplit>,
    pub(crate) area: Option<Rect>,
    pub(crate) tab_id: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct LayoutSplit {
    pub(crate) direction: Option<String>,
    pub(crate) ratio: Option<f64>,
    pub(crate) rect: Option<Rect>,
}

#[derive(Deserialize)]
pub(crate) struct LayoutPane {
    pub(crate) pane_id: Option<String>,
    pub(crate) rect: Option<Rect>,
}

#[derive(Deserialize)]
pub(crate) struct Rect {
    pub(crate) x: i64,
    pub(crate) y: i64,
    pub(crate) width: i64,
    #[serde(default)]
    pub(crate) height: i64,
}

/// Windows PowerShell 5.1 prepends a UTF-8 BOM when piping into a native
/// process's stdin (verified live); serde_json rejects a BOM before `{`.
pub(crate) fn strip_bom(input: &str) -> &str {
    input.trim_start_matches('\u{feff}')
}

/// `<pane_id>\t<ratio>\t<swap>` for the configured edge. Left docking splits
/// the leftmost pane and swaps; right docking splits the rightmost pane with
/// an inverted original-pane ratio and needs no swap. Empty on any failure.
pub fn open_plan(layout_json: &str, dock_right: bool, target_cols: u16) -> String {
    open_plan_inner(layout_json, dock_right, target_cols, MIN_SIDEBAR_SHARE)
}

/// Use the configured columns within Herdr's actual split limits.
pub fn column_open_plan(layout_json: &str, dock_right: bool, target_cols: u16) -> String {
    open_plan_inner(
        layout_json,
        dock_right,
        target_cols,
        PRODUCT_MIN_SIDEBAR_SHARE,
    )
}

fn open_plan_inner(
    layout_json: &str,
    dock_right: bool,
    target_cols: u16,
    min_share: f64,
) -> String {
    let Ok(msg) = serde_json::from_str::<LayoutMsg>(strip_bom(layout_json)) else {
        return String::new();
    };
    let mut best: Option<(&str, &Rect)> = None;
    for pane in &msg.result.layout.panes {
        let (Some(id), Some(rect)) = (pane.pane_id.as_deref(), pane.rect.as_ref()) else {
            continue;
        };
        if !is_flag_safe(id) || rect.width <= 0 {
            continue;
        }
        // The configured edge wins; among a stacked edge column, topmost wins.
        let better = match best {
            None => true,
            Some((_, b)) if dock_right => (rect.x + rect.width, -rect.y) > (b.x + b.width, -b.y),
            Some((_, b)) => (rect.x, rect.y) < (b.x, b.y),
        };
        if better {
            best = Some((id, rect));
        }
    }
    let Some((id, rect)) = best else {
        return String::new();
    };
    let sidebar_share =
        (f64::from(target_cols) / rect.width as f64).clamp(min_share, MAX_SIDEBAR_SHARE);
    let ratio = if dock_right {
        1.0 - sidebar_share
    } else {
        sidebar_share
    };
    format!("{id}\t{ratio:.6}\t{}", !dock_right)
}

/// Width of the whole tab area that owns a pane split. A pane-only divider
/// resize leaves this unchanged; terminal chrome/sidebar changes do not.
pub fn layout_width(layout_json: &str) -> Option<i64> {
    let msg = serde_json::from_str::<LayoutMsg>(strip_bom(layout_json)).ok()?;
    msg.result.layout.area.map(|area| area.width)
}

/// One `herdr pane resize` invocation: which way to move our interior edge and by
/// how much. `amount` is a RATIO delta — herdr adds it to the nearest split's
/// ratio (`layout.rs::resize_focused`: `current_ratio ± delta`), it is NOT
/// columns.
pub struct ResizeStep {
    pub direction: &'static str,
    pub amount: f64,
}

/// Compute the resize step that brings `pane_id` from `term_cols_now` to
/// `term_cols_target` terminal columns, from a `pane layout` JSON.
///
/// The sidebar's interior edge is the divider of some horizontal split: right
/// edge for a left dock, left edge for a right dock. Pick the innermost matching
/// split and convert the column delta into that split's ratio space. `None`
/// when the pane/split can't be found or the pane is already at the target.
pub fn resize_plan(
    layout_json: &str,
    pane_id: &str,
    term_cols_now: u16,
    term_cols_target: u16,
    dock_right: bool,
) -> Option<ResizeStep> {
    resize_plan_inner(
        layout_json,
        pane_id,
        term_cols_now,
        term_cols_target,
        dock_right,
        None,
    )
}

/// Re-assert a preferred column width after the tab's available width changes.
/// The target is exact in the normal range, but yields to the same 15%-50%
/// share bounds used when the sidebar first spawns.
pub fn preferred_resize_plan(
    layout_json: &str,
    pane_id: &str,
    term_cols_now: u16,
    preferred_cols: u16,
    dock_right: bool,
) -> Option<ResizeStep> {
    resize_plan_inner(
        layout_json,
        pane_id,
        term_cols_now,
        preferred_cols,
        dock_right,
        Some(MIN_SIDEBAR_SHARE),
    )
}

/// Product reflow yields only to the host minimum and half-pane limit.
pub fn column_resize_plan(
    layout_json: &str,
    pane_id: &str,
    term_cols_now: u16,
    preferred_cols: u16,
    dock_right: bool,
) -> Option<ResizeStep> {
    resize_plan_inner(
        layout_json,
        pane_id,
        term_cols_now,
        preferred_cols,
        dock_right,
        Some(PRODUCT_MIN_SIDEBAR_SHARE),
    )
}

fn resize_plan_inner(
    layout_json: &str,
    pane_id: &str,
    term_cols_now: u16,
    term_cols_target: u16,
    dock_right: bool,
    min_share: Option<f64>,
) -> Option<ResizeStep> {
    let msg = serde_json::from_str::<LayoutMsg>(strip_bom(layout_json)).ok()?;
    let layout = &msg.result.layout;
    let pane_rect = layout
        .panes
        .iter()
        .find(|p| p.pane_id.as_deref() == Some(pane_id))?
        .rect
        .as_ref()?;
    let divider_x = if dock_right {
        pane_rect.x
    } else {
        pane_rect.x + pane_rect.width
    };
    let split = layout
        .splits
        .iter()
        .filter(|s| s.direction.as_deref() == Some("right"))
        .filter_map(|s| Some((s.rect.as_ref()?, s.ratio?)))
        .filter(|(rect, ratio)| {
            let split_divider = rect.x + (f64::from(rect.width as i32) * ratio).round() as i64;
            rect.x <= pane_rect.x && (split_divider - divider_x).abs() <= 2 && rect.width > 0
        })
        .min_by_key(|(rect, _)| rect.width)?;

    // The pane rect can be a couple of columns wider than the terminal inside
    // it (pane chrome); express the target in rect space. Preferred-width
    // reassertion keeps the content usable at extreme tab widths.
    let chrome = pane_rect.width - i64::from(term_cols_now);
    let requested = i64::from(term_cols_target) + chrome.max(0);
    let target_rect_w = if let Some(min_share) = min_share {
        let min = (split.0.width as f64 * min_share).ceil() as i64;
        let max = (split.0.width as f64 * MAX_SIDEBAR_SHARE).floor() as i64;
        requested.clamp(min, max.max(min))
    } else {
        requested
    };

    let delta = (target_rect_w - pane_rect.width) as f64 / split.0.width as f64;
    if delta.abs() < 0.005 {
        return None;
    }
    Some(ResizeStep {
        direction: match (dock_right, delta > 0.0) {
            (false, true) | (true, false) => "right",
            (false, false) | (true, true) => "left",
        },
        amount: delta.abs(),
    })
}

/// True when the id can be passed as a positional argument to the herdr CLI
/// without any risk of being parsed as a flag.
pub(crate) fn is_flag_safe(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('-')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '.' | '_' | '-'))
}

/// One step of the full-height repair: `below` (a pane under the sidebar,
/// truncating its column) should be re-parented as a down-split of `beside`
/// (the pane toward the tab interior) in `tab`.
pub struct RepairStep {
    pub below: String,
    pub beside: String,
    pub tab: String,
}

/// When `pane_id` is not a full-height column of its tab, find the pane
/// directly below it and the pane beside it toward the tab interior — moving
/// the former under the latter (via a bounce through a temp tab; herdr no-ops
/// same-tab moves) grows the sidebar to full height. `None` when already full
/// height or the layout doesn't match. Called in a loop: each step removes one
/// pane from under the sidebar.
pub fn repair_step(layout_json: &str, pane_id: &str, dock_right: bool) -> Option<RepairStep> {
    let msg = serde_json::from_str::<LayoutMsg>(strip_bom(layout_json)).ok()?;
    let layout = &msg.result.layout;
    let area = layout.area.as_ref()?;
    let tab = layout.tab_id.as_deref().filter(|t| is_flag_safe(t))?;
    let me = layout
        .panes
        .iter()
        .find(|p| p.pane_id.as_deref() == Some(pane_id))?
        .rect
        .as_ref()?;
    if me.height >= area.height {
        return None;
    }
    let find = |pred: &dyn Fn(&Rect) -> bool| -> Option<String> {
        layout
            .panes
            .iter()
            .filter(|p| p.pane_id.as_deref() != Some(pane_id))
            .filter_map(|p| Some((p.pane_id.as_deref()?, p.rect.as_ref()?)))
            .find(|(id, rect)| is_flag_safe(id) && pred(rect))
            .map(|(id, _)| id.to_string())
    };
    let below = find(&|r: &Rect| r.y == me.y + me.height && r.x <= me.x && me.x < r.x + r.width)?;
    let beside = if dock_right {
        find(&|r: &Rect| r.x + r.width == me.x && r.y == me.y)?
    } else {
        find(&|r: &Rect| r.x == me.x + me.width && r.y == me.y)?
    };
    Some(RepairStep {
        below,
        beside,
        tab: tab.to_string(),
    })
}

#[cfg(test)]
mod product_width_tests {
    use super::*;
    #[test]
    fn product_width_yields_only_to_host_split_limits() {
        let geometry = serde_json::json!({"result":{"layout":{
            "panes":[{"pane_id":"p1","rect":{"x":0,"y":0,"width":400,"height":40}}]
        }}})
        .to_string();
        assert_eq!(column_open_plan(&geometry, false, 28), "p1\t0.100000\ttrue");
        assert_eq!(column_open_plan(&geometry, true, 28), "p1\t0.900000\tfalse");
        assert_eq!(open_plan(&geometry, false, 28), "p1\t0.150000\ttrue");
        for (width, current, target, expected) in [(400, 60, 28, 0.05), (40, 20, 28, 0.0)] {
            let geometry = serde_json::json!({"result":{"layout":{
                "panes":[{"pane_id":"p1","rect":{"x":0,"y":0,"width":current,"height":40}}],
                "splits":[{"direction":"right","ratio":current as f64 / width as f64,"rect":{"x":0,"y":0,"width":width,"height":40}}]
            }}}).to_string();
            let step = column_resize_plan(&geometry, "p1", current, target, false);
            if expected == 0.0 {
                assert!(step.is_none());
            } else {
                let step = step.unwrap();
                assert_eq!(step.direction, "left");
                assert!((step.amount - expected).abs() < 1e-6);
            }
        }
    }
}
