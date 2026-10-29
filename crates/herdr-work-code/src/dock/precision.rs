//! Exact column sizing through an explicit split path, without moving focus.
use super::{LayoutMsg, strip_bom};

pub struct PreciseResize {
    pub path: Vec<bool>,
    pub ratio: f64,
    pub chrome: u16,
    pub changed: bool,
}

pub fn precise_resize_plan(
    json: &str,
    owner: &str,
    current: u16,
    target: u16,
    right: bool,
    chrome: Option<u16>,
) -> Option<PreciseResize> {
    let message: LayoutMsg = serde_json::from_str(strip_bom(json)).ok()?;
    let layout = message.result.layout;
    let me = layout
        .panes
        .iter()
        .find(|pane| pane.pane_id.as_deref() == Some(owner))?
        .rect
        .as_ref()?;
    let edge = if right { me.x } else { me.x + me.width };
    let split = layout
        .splits
        .iter()
        .filter(|split| split.direction.as_deref() == Some("right"))
        .filter(|split| {
            split
                .rect
                .as_ref()
                .zip(split.ratio)
                .is_some_and(|(area, ratio)| {
                    area.width > 0
                        && area.x <= me.x
                        && (area.x + (area.width as f64 * ratio).round() as i64 - edge).abs() <= 2
                })
        })
        .min_by_key(|split| split.rect.as_ref().map(|area| area.width))?;
    let split_area = *split.rect.as_ref()?;
    let chrome = chrome.unwrap_or((me.width - i64::from(current)).max(0) as u16);
    let requested = i64::from(target) + i64::from(chrome);
    if requested <= 0 || requested >= split_area.width {
        return None;
    }
    let mut region = layout.area?;
    let mut path = Vec::new();
    while region != split_area {
        if path.len() > 64 {
            return None;
        }
        let parent = layout
            .splits
            .iter()
            .find(|split| split.rect.as_ref() == Some(&region))?;
        let ratio = parent.ratio?;
        let second = match parent.direction.as_deref()? {
            "right" => {
                let first = (region.width as f64 * ratio).round() as i64;
                let second = split_area.x >= region.x + first;
                if second {
                    region.x += first;
                    region.width -= first;
                } else {
                    region.width = first;
                }
                second
            }
            "down" => {
                let first = (region.height as f64 * ratio).round() as i64;
                let second = split_area.y >= region.y + first;
                if second {
                    region.y += first;
                    region.height -= first;
                } else {
                    region.height = first;
                }
                second
            }
            _ => return None,
        };
        path.push(second);
    }
    let share = requested as f64 / split_area.width as f64;
    Some(PreciseResize {
        path,
        ratio: if right { 1.0 - share } else { share },
        chrome,
        changed: me.width != requested,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_sizing_targets_only_the_inner_split_and_accounts_for_pane_chrome() {
        for right in [false, true] {
            let json=serde_json::json!({"result":{"layout":{
                "area":{"x":0,"y":0,"width":240,"height":50},
                "panes":[{"pane_id":"owner","rect":{"x":if right {212}else{100},"y":0,"width":28,"height":50}}],
                "splits":[
                    {"direction":"right","ratio":100.0/240.0,"rect":{"x":0,"y":0,"width":240,"height":50}},
                    {"direction":"right","ratio":if right {112.0/140.0}else{28.0/140.0},"rect":{"x":100,"y":0,"width":140,"height":50}}
                ]}}}).to_string();
            let plan = precise_resize_plan(&json, "owner", 26, 4, right, None).unwrap();
            assert_eq!(plan.path, vec![true]);
            assert!((plan.ratio - (if right { 134.0 / 140.0 } else { 6.0 / 140.0 })).abs() < 1e-9);
            assert_eq!(plan.chrome, 2);
            assert!(plan.changed);
        }
    }

    #[test]
    fn cached_chrome_repairs_a_viewport_below_the_pty_minimum() {
        for width in [4, 5, 6] {
            let json = serde_json::json!({"result":{"layout":{
                "area":{"x":0,"y":0,"width":160,"height":50},
                "panes":[{"pane_id":"owner","rect":{"x":0,"y":0,"width":width,"height":50}}],
                "splits":[{"direction":"right","ratio":f64::from(width)/160.0,"rect":{"x":0,"y":0,"width":160,"height":50}}]
            }}}).to_string();
            // The PTY still reports four columns when the bordered viewport
            // has only two or three. Its size cannot measure the chrome now.
            let plan = precise_resize_plan(&json, "owner", 4, 4, false, Some(2)).unwrap();
            assert!((plan.ratio - 6.0 / 160.0).abs() < 1e-9);
            assert_eq!(plan.changed, width != 6);
        }
    }
}
