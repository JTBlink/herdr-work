//! Reparent an existing sidebar using the original preview replay machinery.
use super::*;

/// Make the product sidebar an edge column inside its safe working region.
/// Caller holds the product layout lock. Reuse the preview journal/replay so
/// working processes and their split ratios survive reparenting and failures.
pub fn dock_column(
    profile: &Path,
    owner: &str,
    cwd: &Path,
    tab: &str,
    right: bool,
    width: u16,
) -> Result<bool, String> {
    let mut env = RealEnv {
        profile: Some(profile.to_path_buf()),
    };
    let protected = protect_independent_plugins(&mut env, owner, tab)?;
    dock_column_with(&mut env, owner, cwd, tab, right, width, &protected)
}

pub(super) fn dock_column_with(
    env: &mut impl Env,
    owner: &str,
    cwd: &Path,
    tab: &str,
    right: bool,
    width: u16,
    protected: &BTreeSet<String>,
) -> Result<bool, String> {
    // A live preview owns the layout until its normal restore completes.
    if env.load(tab)?.is_some() {
        return Ok(false);
    }
    let layout: Layout = serde_json::from_value(
        env.call("pane.layout", json!({"pane_id": owner}))?["layout"].clone(),
    )
    .map_err(|error| error.to_string())?;
    let full = tree(&layout, layout.area)?;
    let scope = full
        .working_scope(owner, protected)
        .ok_or("missing sidebar region")?;
    let Some(work) = scope.prune(&|id| id != owner) else {
        return Ok(false);
    };
    let me = layout
        .panes
        .iter()
        .find(|p| p.pane_id == owner)
        .ok_or("missing sidebar")?;
    let members = layout
        .panes
        .iter()
        .filter(|p| scope.contains(&p.pane_id))
        .collect::<Vec<_>>();
    let left = members
        .iter()
        .map(|p| p.rect.x)
        .min()
        .ok_or("empty sidebar region")?;
    let top = members
        .iter()
        .map(|p| p.rect.y)
        .min()
        .ok_or("empty sidebar region")?;
    let edge = members
        .iter()
        .map(|p| p.rect.x + p.rect.width)
        .max()
        .unwrap();
    let bottom = members
        .iter()
        .map(|p| p.rect.y + p.rect.height)
        .max()
        .unwrap();
    if me.rect.y == top
        && me.rect.height == bottom - top
        && if right {
            me.rect.x + me.rect.width == edge
        } else {
            me.rect.x == left
        }
    {
        return Ok(false);
    }
    let share = (f64::from(width) / (edge - left).max(1) as f64)
        .clamp(crate::dock::PRODUCT_MIN_SIDEBAR_SHARE, 0.5);
    let sidebar = Box::new(Node::Pane(owner.into()));
    let work = Box::new(work);
    let desired = Node::Split {
        direction: "right".into(),
        ratio: if right { 1.0 - share } else { share },
        first: if right { work.clone() } else { sidebar.clone() },
        second: if right { sidebar } else { work },
    };
    let plan = prepare_plan(env, owner, cwd, tab, desired)?;
    match restore_with(env, &plan, Trigger::Recover)? {
        Outcome::Restored { unrestored: 0 } => Ok(true),
        _ => Err("侧栏布局未完全恢复；工作窗格保留在恢复标签页中".into()),
    }
}
